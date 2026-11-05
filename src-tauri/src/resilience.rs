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

    let mutex_name: Vec<u16> = "Global\\CP_Engine_SingleInstance_Mutex\0"
        .encode_utf16()
        .collect();

    unsafe {
        let mut handle = CreateMutexW(std::ptr::null_mut(), 1, mutex_name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if is_restarted {
                // Wait up to 3000ms for parent process to fully exit and release the kernel mutex
                for _ in 0..60 {
                    if !handle.is_null() {
                        CloseHandle(handle);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    handle = CreateMutexW(std::ptr::null_mut(), 1, mutex_name.as_ptr());
                    if GetLastError() != ERROR_ALREADY_EXISTS {
                        INSTANCE_MUTEX = handle;
                        return true; // Successfully acquired! Proceed with startup.
                    }
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
            let res = RegisterApplicationRestart(cmd_line.as_ptr(), 0);
            if res == 0 {
                eprintln!("[CP Resilience] Windows WER application restart registered successfully (ExitCode 0x0)");
            } else {
                eprintln!("[CP Resilience] RegisterApplicationRestart returned status: 0x{:X}", res);
            }
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

                if let Err(err) = result {
                    eprintln!(
                        "[CP Supervisor] Worker '{}' caught unexpected panic ({:?}). Reviving worker in 1s...",
                        name, err
                    );
                } else {
                    eprintln!(
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
