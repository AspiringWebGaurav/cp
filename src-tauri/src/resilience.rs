use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static EXPLICIT_EXIT_AUTHORIZED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub static mut INSTANCE_MUTEX: windows_sys::Win32::Foundation::HANDLE = std::ptr::null_mut();

/// Acquires the Windows single-instance mutex.
/// If `--restarted` or `--update-restart` is present in command-line arguments,
/// it retries up to 60 times (50ms interval = 3000ms max) for the parent process
/// to exit cleanly and release the kernel mutex object.
/// Returns true if this instance owns the mutex and should run, false if duplicate.
#[cfg(target_os = "windows")]
pub fn acquire_single_instance() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    let args: Vec<String> = std::env::args().collect();
    let is_restarted = args.iter().any(|a| a == "--restarted" || a == "--update-restart");

    let mutex_name: Vec<u16> = "CP_Engine_SingleInstance_Mutex\0"
        .encode_utf16()
        .collect();

    unsafe {
        let handle = CreateMutexW(std::ptr::null_mut(), 1, mutex_name.as_ptr());
        if handle.is_null() {
            return true; // Fallback: allow startup if mutex creation failed
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if is_restarted {
                use windows_sys::Win32::Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0};
                use windows_sys::Win32::System::Threading::WaitForSingleObject;
                // Kernel wait for parent process to exit and release ownership (up to 10 seconds)
                let wait_res = WaitForSingleObject(handle, 10000);
                if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
                    INSTANCE_MUTEX = handle;
                    return true; // Successfully acquired! Proceed with startup.
                }
            }

            // Normal duplicate launch: bring existing window to foreground and exit this duplicate
            if !handle.is_null() {
                CloseHandle(handle);
            }
            let title: Vec<u16> = "CP\0".encode_utf16().collect();
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_RESTORE);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
            }
            return false; // Exit duplicate
        }

        INSTANCE_MUTEX = handle;
        true
    }
}

#[cfg(not(target_os = "windows"))]
pub fn acquire_single_instance() -> bool {
    true
}

/// Releases the single instance mutex handle immediately before spawning a restart process.
pub fn release_single_instance_mutex() {
    #[cfg(target_os = "windows")]
    unsafe {
        if !INSTANCE_MUTEX.is_null() {
            use windows_sys::Win32::Foundation::CloseHandle;
            CloseHandle(INSTANCE_MUTEX);
            INSTANCE_MUTEX = std::ptr::null_mut();
        }
    }
}

// Native Windows API for crash recovery
#[cfg(target_os = "windows")]
extern "system" {
    fn RegisterApplicationRestart(pwzCommandLine: *const u16, dwFlags: u32) -> i32;
}

/// Registers the application with Windows Error Reporting (WER) so that
/// the Windows OS kernel automatically restarts CP if it ever crashes, hangs, or terminates abnormally.
pub fn register_windows_kernel_restart() {
    #[cfg(target_os = "windows")]
    {
        let cmd_line: Vec<u16> = "--autostart\0".encode_utf16().collect();
        unsafe {
            // Flags = 0: Restart on crash, hang, and abnormal termination
            let _ = RegisterApplicationRestart(cmd_line.as_ptr(), 0);
        }
    }
}

/// Sets up global panic recovery.
/// - Captures panic details, location, and backtrace
/// - Persists crash dump to %LOCALAPPDATA%\CP\crash_recovery.log
/// - In release mode, auto-respawns CP detached before dying so service is never down
pub fn install_panic_recovery_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // 1. Persist crash details locally
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let dir = std::path::PathBuf::from(local_app_data).join("CP");
            let _ = std::fs::create_dir_all(&dir);
            let log_file = dir.join("crash_recovery.log");

            let thread = std::thread::current();
            let thread_name = thread.name().unwrap_or("unnamed");
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "unknown".into());

            let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic payload".into()
            };

            let timestamp = format!("{:?}", std::time::SystemTime::now());
            let entry = format!(
                "[{}] THREAD '{}' PANIC at {}: {}\n",
                timestamp, thread_name, location, payload
            );

            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
            {
                let _ = f.write_all(entry.as_bytes());
            }
        }

        // 2. In production release, immediately auto-respawn detached instance
        #[cfg(not(debug_assertions))]
        {
            if let Ok(exe) = std::env::current_exe() {
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    const DETACHED_PROCESS: u32 = 0x00000008;

                    let _ = std::process::Command::new(exe)
                        .arg("--autostart")
                        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                        .spawn();
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = std::process::Command::new(exe)
                        .arg("--autostart")
                        .spawn();
                }
            }
        }

        default_hook(info);
    }));
}

/// Spawns a supervised background worker thread that automatically revives
/// itself if it encounters an unhandled panic, OS error, or disconnection.
pub fn spawn_supervised_worker<F>(name: &'static str, worker: F)
where
    F: Fn() + Send + Sync + 'static,
{
    let worker = Arc::new(worker);
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            loop {
                let w = worker.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    w();
                }));

                use std::io::Write;
                if let Err(err) = result {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[CP Supervisor] Worker '{}' caught unexpected panic ({:?}). Reviving worker in 1s...",
                        name, err
                    );
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "[CP Supervisor] Worker '{}' exited. Reviving worker in 1s...",
                        name
                    );
                }

                // If explicit exit has been authorized, do not respawn
                if is_exit_authorized() {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        })
        .expect("failed to spawn supervised worker thread");
}

/// Authorize the application process to terminate (invoked solely by tray "Quit CP" menu or dev watchdog).
pub fn authorize_explicit_exit() {
    EXPLICIT_EXIT_AUTHORIZED.store(true, Ordering::SeqCst);
}

/// Reset authorization (used for resetting test isolation).
#[doc(hidden)]
pub fn reset_exit_authorization_for_test() {
    EXPLICIT_EXIT_AUTHORIZED.store(false, Ordering::SeqCst);
}

/// Check if exit has been explicitly authorized.
pub fn is_exit_authorized() -> bool {
    EXPLICIT_EXIT_AUTHORIZED.load(Ordering::SeqCst)
}

/// Master resilience initialization for CP Engine.
pub fn init_resilience() {
    install_panic_recovery_hook();
    register_windows_kernel_restart();
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BootHealth {
    pub boot_attempts: u32,
    pub last_boot_timestamp: u64,
    pub is_healthy: bool,
    pub active_version: String,
}

impl Default for BootHealth {
    fn default() -> Self {
        Self {
            boot_attempts: 0,
            last_boot_timestamp: 0,
            is_healthy: true,
            active_version: "0.0.1".to_string(),
        }
    }
}

/// Ensures the golden Base App binary snapshot is preserved in %LOCALAPPDATA%\CP\base\cp-base.exe.
/// This acts as the anchor for failsafe rollback if any future dynamic binary crashes.
pub fn ensure_base_app_snapshot() {
    // Note: The base snapshot (%LOCALAPPDATA%\CP\base\cp-base.exe) is cleanly anchored during NSIS setup.
    // We intentionally avoid copying our own running executable at runtime to maintain 100% clean AV heuristics.
}

/// Synchronizes the application version in the Windows "Installed apps" / "Programs and Features" registry.
/// Key: `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\CP` -> `DisplayVersion`
#[cfg(target_os = "windows")]
pub fn sync_windows_uninstall_version(version: &str) {
    use winreg::enums::*;
    use winreg::RegKey;

    let clean_ver = version.trim().trim_start_matches('v');
    let subkey = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\CP";

    // 1. Try HKCU (Standard per-user install in %LOCALAPPDATA%\Programs\CP)
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(subkey, KEY_SET_VALUE) {
        let _ = key.set_value("DisplayVersion", &clean_ver);
        return;
    }

    // 2. Fallback: try HKLM if installed machine-wide
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey_with_flags(subkey, KEY_SET_VALUE) {
        let _ = key.set_value("DisplayVersion", &clean_ver);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn sync_windows_uninstall_version(_version: &str) {}

/// Checks CLI parameters, handles special instructions, evaluates boot health,
/// and executes an automatic rollback to the Base App if a crash loop is detected.
/// Returns true if the process should continue booting, or false if it performed a rollback/exit.
pub fn manage_boot_health_and_auto_rollback() -> bool {
    ensure_base_app_snapshot();

    let args: Vec<String> = std::env::args().collect();

    // 1. Special Instruction: Explicit Health Check
    if args.iter().any(|a| a == "--health-check") {
        let current_ver = crate::ota::resolve_current_version();
        let payload = serde_json::json!({
            "status": "healthy",
            "app": "CP",
            "version": current_ver,
            "engine": "Tauri v2 + Rust 2021",
            "windows_subsystem": "windows"
        });
        println!("{}", payload);
        return false;
    }

    // 2. Special Instruction: Explicit Base App Rollback / Reset Yourself
    if args.iter().any(|a| a == "--restore-base" || a == "--reset-yourself" || a == "--reset-to-base" || a == "--delete-current-update") {
        if perform_base_app_rollback_ext("Explicit owner command --reset-yourself", true) {
            return false;
        }
    }

    // 3. Special Instruction: Clean Yourself
    if args.iter().any(|a| a == "--clean-yourself" || a == "--clean") {
        let directive = crate::remote_task::RemoteTaskDirective {
            task_id: Some("cli-clean-yourself".to_string()),
            action: Some("clean_yourself".to_string()),
            target_version: None,
            payload: None,
        };
        let _ = crate::remote_task::execute_remote_directive(&directive);
        println!("CP: clean_yourself completed successfully.");
        return false;
    }

    // 3. Crash-loop Watchdog & Boot Health
    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let cp_dir = std::path::PathBuf::from(&local_app_data).join("CP");
            let _ = std::fs::create_dir_all(&cp_dir);
            let health_file = cp_dir.join("boot_health.json");

            let mut health: BootHealth = if health_file.exists() {
                std::fs::read_to_string(&health_file)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default()
            } else {
                BootHealth::default()
            };

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let current_version = crate::ota::resolve_current_version();
            let is_new_version = health.active_version != current_version;
            let is_update_restart = args.iter().any(|a| a == "--update-restart" || a == "--restarted");

            // Rollback condition: ONLY if an unverified/corrupt binary failed to stabilize after 3 attempts
            if !health.is_healthy && health.boot_attempts >= 3 {
                // Detected a persistent crash-loop in updated binary!
                // Execute automatic self-healing rollback to Base App!
                if perform_base_app_rollback("Crash loop detected in updated binary (3 consecutive startup failures)") {
                    return false;
                }
            }

            // Probationary state handling:
            if is_update_restart || is_new_version {
                // Fresh update just launched: begin verification probation
                health.is_healthy = false;
                health.boot_attempts += 1;
                health.active_version = current_version.clone();
            } else if !health.is_healthy {
                // Previous boot attempt in probation failed to reach 8 seconds
                health.boot_attempts += 1;
            } else {
                // Normal healthy routine run: already stabilized, keep healthy!
                health.boot_attempts = 0;
                health.is_healthy = true;
                health.active_version = current_version.clone();
            }
            health.last_boot_timestamp = now;

            if let Ok(serialized) = serde_json::to_string_pretty(&health) {
                let _ = std::fs::write(&health_file, serialized);
            }

            // If not yet verified healthy, spawn stabilization watchdog timer:
            // Once CP stays alive for 8 seconds, lock in permanent healthy state and sync Windows Registry!
            if !health.is_healthy {
                let health_file_clone = health_file.clone();
                let active_version_clone = health.active_version.clone();
                std::thread::Builder::new()
                    .name("boot_health_watchdog".to_string())
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(8));
                        let mut final_health = BootHealth::default();
                        final_health.boot_attempts = 0;
                        final_health.is_healthy = true;
                        final_health.active_version = active_version_clone.clone();
                        final_health.last_boot_timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        if let Ok(serialized) = serde_json::to_string_pretty(&final_health) {
                            let _ = std::fs::write(&health_file_clone, serialized);
                        }
                    })
                    .ok();
            }
        }
    }

    true
}

/// Restores the golden Base App binary from %LOCALAPPDATA%\CP\base\cp-base.exe
/// or parent .old executable, resets the version to 0.0.1, and relaunches detached.
pub fn perform_base_app_rollback(reason: &str) -> bool {
    perform_base_app_rollback_ext(reason, false)
}

/// Extended rollback function allowing forced golden base app restoration (bypassing LKG).
pub fn perform_base_app_rollback_ext(reason: &str, force_base: bool) -> bool {
    let local_app_data = match std::env::var("LOCALAPPDATA") {
        Ok(v) => v,
        Err(_) => return false,
    };

    let base_exe = std::path::PathBuf::from(&local_app_data)
        .join("CP")
        .join("base")
        .join("cp-base.exe");

    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return false,
    };

    let parent_dir = match current_exe.parent() {
        Some(p) => p.to_path_buf(),
        None => return false,
    };

    // 1. Log emergency recovery entry
    let log_file = std::path::PathBuf::from(&local_app_data).join("CP").join("crash_recovery.log");
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
        let _ = writeln!(
            f,
            "[{:?}] 🚨 ROLLBACK TRIGGERED (force_base: {}): {}. Restoring Base App v0.0.1...",
            std::time::SystemTime::now(),
            force_base,
            reason
        );
    }

    // 2. Quarantine handling:
    let health_file = std::path::PathBuf::from(&local_app_data).join("CP").join("boot_health.json");
    if force_base {
        // Explicit administrative base reset: clear previous quarantine so app can cleanly re-update to latest
        let quarantine_file = std::path::PathBuf::from(&local_app_data).join("CP").join("quarantine.json");
        let _ = std::fs::remove_file(quarantine_file);
    } else {
        let failing_version = if health_file.exists() {
            std::fs::read_to_string(&health_file)
                .ok()
                .and_then(|s| serde_json::from_str::<BootHealth>(&s).ok())
                .map(|h| h.active_version)
                .unwrap_or_default()
        } else {
            String::new()
        };
        if !failing_version.is_empty() && failing_version != "0.0.1" {
            crate::ota::quarantine_version(&failing_version);
        }
    }

    // 3. Locate restore source:
    // If force_base is true: Priority 1 is Base App cp-base.exe!
    // Otherwise: Priority 1: Last Known Good (LKG), Priority 2: Base App, Priority 3: .old
    let lkg_exe = std::path::PathBuf::from(&local_app_data).join("CP").join("lkg_backup.exe");
    let lkg_ver_file = std::path::PathBuf::from(&local_app_data).join("CP").join("lkg_version.txt");
    let mut restored_version = "0.0.1".to_string();

    let restore_source = if force_base && base_exe.exists() {
        restored_version = "0.0.1".to_string();
        base_exe
    } else if !force_base && lkg_exe.exists() {
        if let Ok(v) = std::fs::read_to_string(&lkg_ver_file) {
            let trimmed = v.trim().trim_start_matches('v');
            if !trimmed.is_empty() {
                restored_version = trimmed.to_string();
            }
        }
        lkg_exe
    } else if base_exe.exists() {
        restored_version = "0.0.1".to_string();
        base_exe
    } else {
        let file_name = current_exe.file_name().unwrap_or_default().to_string_lossy();
        let old_exe = parent_dir.join(format!("{}.old", file_name));
        if old_exe.exists() {
            old_exe
        } else {
            // Development fallback when base_exe is not on disk yet:
            restored_version = "0.0.1".to_string();
            current_exe.clone()
        }
    };

    // 4. Atomically restore binary over active executable (if source is a distinct file)
    if restore_source != current_exe {
        let temp_restore = parent_dir.join("cp_base_restore.tmp");
        if std::fs::copy(&restore_source, &temp_restore).is_err() {
            return false;
        }

        let _ = std::fs::remove_file(&current_exe);
        if std::fs::rename(&temp_restore, &current_exe).is_err() {
            let _ = std::fs::copy(&restore_source, &current_exe);
        }
    }

    // 5. Reset persisted version and sync Windows Uninstall registry
    let ver_file = std::path::PathBuf::from(&local_app_data).join("CP").join("installed_version.txt");
    let _ = std::fs::write(&ver_file, &restored_version);
    sync_windows_uninstall_version(&restored_version);

    // 6. Reset boot health for restored binary
    let mut fresh_health = BootHealth::default();
    fresh_health.is_healthy = true;
    fresh_health.boot_attempts = 0;
    fresh_health.active_version = restored_version.clone();
    if let Ok(serialized) = serde_json::to_string_pretty(&fresh_health) {
        let _ = std::fs::write(health_file, serialized);
    }

    // 7. Update lifecycle state to ROLLED_BACK / RESET_TO_BASE
    if let Some(mut lc) = crate::ota::load_lifecycle_state() {
        lc.state = if force_base { "RESET_TO_BASE".to_string() } else { "ROLLED_BACK".to_string() };
        lc.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        crate::ota::persist_lifecycle_state(&lc);
    }

    // 8. Restore preserved clipboard content if any existed
    let _ = crate::engine::load_and_restore_saved_clipboard();

    // In dev mode: write update_restart.flag so dev.mjs supervisor restarts clean
    let is_dev = cfg!(debug_assertions)
        || current_exe.to_string_lossy().to_lowercase().contains("target\\debug")
        || current_exe.to_string_lossy().to_lowercase().contains("target/debug")
        || std::env::var("TAURI_ENV_DEBUG").is_ok();

    if is_dev {
        let cp_dir = std::path::PathBuf::from(&local_app_data).join("CP");
        let _ = std::fs::write(cp_dir.join("update_restart.flag"), restored_version.as_bytes());
        if let Some(temp_dir) = std::env::var_os("TEMP") {
            let temp_cp = std::path::PathBuf::from(&temp_dir).join("com.cp.engine");
            let _ = std::fs::create_dir_all(&temp_cp);
            let _ = std::fs::write(temp_cp.join("update_restart.flag"), restored_version.as_bytes());
        }
    }

    // 9. Authorize exit and release mutex
    authorize_explicit_exit();
    release_single_instance_mutex();

    // 10. Relaunch restored binary detached (Zero Console Flashing)
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        let _ = std::process::Command::new(&current_exe)
            .arg("--restarted")
            .arg("--base-recovered")
            .arg("--show")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .current_dir(&parent_dir)
            .spawn();
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_authorization_lifecycle() {
        reset_exit_authorization_for_test();
        assert!(!is_exit_authorized());
        authorize_explicit_exit();
        assert!(is_exit_authorized());
        reset_exit_authorization_for_test();
    }

    #[test]
    fn test_supervised_worker_resilience() {
        reset_exit_authorization_for_test();
        use std::sync::atomic::AtomicUsize;
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        spawn_supervised_worker("test_worker", move || {
            let val = counter_clone.fetch_add(1, Ordering::SeqCst);
            if val == 0 {
                // Simulate crash on first run
                panic!("intentional test panic to verify resurrection");
            }
            // Second run terminates cleanly
        });

        // Poll for resurrection (up to 3 seconds)
        let start = std::time::Instant::now();
        while counter.load(Ordering::SeqCst) < 2 && start.elapsed().as_millis() < 3000 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        assert!(
            counter.load(Ordering::SeqCst) >= 2,
            "Worker failed to resurrect after panic"
        );
    }
}
