pub mod engine;
pub mod hardware;
pub mod ota;
pub mod interceptor;
pub mod autostart;
pub mod resilience;

use hardware::{probe_system_info, record_completed_transfer, reset_real_stats, SystemInfo};
use engine::{run_benchmark, BenchmarkResult, inspect_clipboard_files, ClipboardFileInfo};
use ota::{check_latest_release, ReleaseManifest};
use std::path::PathBuf;

#[tauri::command]
fn is_autostart_enabled() -> bool {
    autostart::is_autostart_registered()
}

#[tauri::command]
fn set_autostart_enabled(enabled: bool) -> Result<bool, String> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = std::path::PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&dir);
        let pref_file = dir.join("autostart_disabled.flag");
        if enabled {
            let _ = std::fs::remove_file(pref_file);
        } else {
            let _ = std::fs::write(pref_file, "1");
        }
    }
    autostart::set_autostart_registered(enabled)?;
    Ok(enabled)
}

#[tauri::command]
fn get_system_info() -> SystemInfo {
    probe_system_info()
}

#[tauri::command]
fn get_clipboard_files() -> Option<ClipboardFileInfo> {
    inspect_clipboard_files()
}

#[tauri::command]
fn record_transfer(bytes: u64) -> SystemInfo {
    record_completed_transfer(bytes);
    probe_system_info()
}

#[tauri::command]
fn reset_stats() -> SystemInfo {
    reset_real_stats();
    probe_system_info()
}

#[tauri::command]
fn run_transfer_benchmark(size_mb: u64) -> BenchmarkResult {
    let result = run_benchmark(size_mb);
    record_completed_transfer(result.bytes_copied);
    result
}

#[tauri::command]
async fn execute_copy_transfer(
    app: tauri::AppHandle,
    source_path: String,
    destination_dir: String,
) -> Result<SystemInfo, String> {
    use tauri::Emitter;

    // Concurrency Lock: Prevent multiple parallel copy executions
    if !engine::try_acquire_transfer() {
        return Err("A file transfer is already actively running".to_string());
    }

    struct TransferLockGuard;
    impl Drop for TransferLockGuard {
        fn drop(&mut self) {
            engine::release_transfer();
        }
    }
    let _guard = TransferLockGuard;

    let src = PathBuf::from(&source_path);
    if !src.exists() {
        return Err(format!("Source path does not exist: {}", source_path));
    }

    let dst_dir = PathBuf::from(&destination_dir);
    if !dst_dir.exists() {
        let _ = std::fs::create_dir_all(&dst_dir);
    }

    // Generate unique, collision-proof destination path (handles " - Copy" on same folder or existing name)
    let dst = engine::generate_unique_destination_path(&src, &dst_dir);

    let app_handle = app.clone();
    let bytes_copied = tokio::task::spawn_blocking(move || {
        let mut last_emit = std::time::Instant::now();

        engine::copy_path_streaming(&src, &dst, |copied, total, speed_gb| {
            let now = std::time::Instant::now();
            let should_emit = copied == total || now.duration_since(last_emit).as_millis() >= 40;

            if should_emit {
                last_emit = now;
                let pct = if total > 0 {
                    (copied as f64 / total as f64) * 100.0
                } else {
                    100.0
                };

                let speed_str = if speed_gb >= 1.0 {
                    format!("{:.2} GB/s", speed_gb)
                } else {
                    format!("{:.1} MB/s", speed_gb * 1024.0)
                };

                let remaining_bytes = total.saturating_sub(copied);
                let speed_bytes_per_sec = (speed_gb * 1024.0 * 1024.0 * 1024.0).max(1.0);
                let eta_secs = (remaining_bytes as f64 / speed_bytes_per_sec).max(0.0);
                let eta_str = if eta_secs < 1.0 {
                    format!("{:.1}s", eta_secs)
                } else if eta_secs < 60.0 {
                    format!("{}s", eta_secs as u64)
                } else {
                    format!("{}m {}s", (eta_secs / 60.0) as u64, (eta_secs as u64) % 60)
                };

                let update = engine::ProgressUpdate {
                    bytes_copied: copied,
                    total_bytes: total,
                    formatted_copied: crate::hardware::format_bytes(copied),
                    formatted_total: crate::hardware::format_bytes(total),
                    percentage: (pct * 10.0).round() / 10.0,
                    speed: speed_str,
                    eta: eta_str,
                    is_finished: copied == total,
                };

                let _ = app_handle.emit("transfer-progress", update);
            }
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    record_completed_transfer(bytes_copied);
    Ok(probe_system_info())
}

#[tauri::command]
fn pause_transfer(paused: bool) {
    engine::set_transfer_paused(paused);
}

#[tauri::command]
fn cancel_transfer() {
    engine::cancel_current_transfer();
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.emit("show-console", ());
    }
}

#[tauri::command]
fn get_app_version() -> String {
    ota::resolve_current_version()
}

#[tauri::command]
async fn check_update(current_version: String) -> Result<Option<ReleaseManifest>, String> {
    check_latest_release(&current_version).await
}

#[tauri::command]
fn check_and_consume_pending_paste() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let arg_has_paste = args.iter().any(|a| a == "--auto-paste");
    let file_has_paste = if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let flag = std::path::PathBuf::from(local_app_data).join("CP").join("pending_paste.flag");
        if flag.exists() {
            let _ = std::fs::remove_file(flag);
            true
        } else {
            false
        }
    } else {
        false
    };
    arg_has_paste || file_has_paste
}

#[tauri::command]
fn verify_update_signature(
    container_bytes: Vec<u8>,
    signature: String,
) -> Result<bool, String> {
    ota::verify_binary(&container_bytes, Some(&signature)).map(|_| true)
}

#[tauri::command]
fn write_update_staging_file(file_name: String, bytes: Vec<u8>) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        let temp_dir = std::env::temp_dir().join("com.cp.engine");
        let _ = std::fs::create_dir_all(&temp_dir);
        let target_file = temp_dir.join(&file_name);
        let part_file = temp_dir.join(format!("{}.part", file_name));

        // 1. Write to temporary .part file first
        {
            let mut file = std::fs::File::create(&part_file)
                .map_err(|e| format!("Could not create staging part file: {}", e))?;
            file.write_all(&bytes)
                .map_err(|e| format!("Could not write staging file: {}", e))?;
            file.flush()
                .map_err(|e| format!("Could not flush staging file: {}", e))?;
        }

        // 2. Atomic swap to final staged filename
        if target_file.exists() {
            let _ = std::fs::remove_file(&target_file);
        }
        std::fs::rename(&part_file, &target_file)
            .map_err(|e| format!("Could not finalize staging file: {}", e))?;

        Ok(target_file.to_string_lossy().to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (file_name, bytes);
        Err("In-process staging is supported on Windows".to_string())
    }
}

#[tauri::command]
fn apply_in_process_update(
    app: tauri::AppHandle,
    staging_path: String,
    target_version: String,
    pending_paste: Option<bool>,
) -> Result<(), String> {
    ota::apply_update_and_restart(
        app,
        std::path::PathBuf::from(staging_path),
        target_version,
        pending_paste.unwrap_or(false),
    )
}

#[tauri::command]
async fn download_update_container(
    app: tauri::AppHandle,
    url: String,
) -> Result<Vec<u8>, String> {
    ota::download_binary_container(Some(&app), &url).await
}

#[cfg(target_os = "windows")]
static IS_POSITION_LOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "windows")]
unsafe extern "system" fn fixed_position_subclass(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    _uid_subclass: usize,
    _ref_data: usize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONDBLCLK, WM_SYSCOMMAND,
        WM_WINDOWPOSCHANGING, HTCAPTION, HTBORDER, SC_MOVE, SWP_NOMOVE, WINDOWPOS,
    };
    use windows_sys::Win32::UI::Shell::DefSubclassProc;

    // 1. Block mouse drag on titlebar:
    // If Windows tests mouse location on caption, return HTBORDER so Windows treats it as a static border.
    // Close button [X] returns HTCLOSE (20) and remains fully functional.
    if msg == WM_NCHITTEST {
        let hit = DefSubclassProc(hwnd, msg, wparam, lparam);
        if hit == HTCAPTION as isize {
            return HTBORDER as isize;
        }
        return hit;
    }

    // 2. Block mouse down and double click on titlebar (HTCAPTION = 2)
    if (msg == WM_NCLBUTTONDOWN || msg == WM_NCLBUTTONDBLCLK) && wparam == HTCAPTION as usize {
        return 0;
    }

    // 3. Block system menu / keyboard "Move" command
    if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_MOVE as usize {
        return 0;
    }

    // 4. Freeze position once centered: force SWP_NOMOVE flag so nothing can relocate the window
    if IS_POSITION_LOCKED.load(std::sync::atomic::Ordering::Relaxed) && msg == WM_WINDOWPOSCHANGING {
        let pos = &mut *(lparam as *mut WINDOWPOS);
        pos.flags |= SWP_NOMOVE;
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

            // Start Windows Explorer Global Interception
            interceptor::start_explorer_interceptor(app.handle().clone());

            if let Some(main_win) = app.get_webview_window("main") {
                // Ensure window is centered on screen
                let _ = main_win.center();

                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = main_win.set_icon(icon);
                }

                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        GetWindowLongW, SetWindowLongW, SetWindowPos,
                        GWL_STYLE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
                        GetSystemMenu, DeleteMenu, MF_BYCOMMAND,
                        SC_MINIMIZE, SC_MAXIMIZE, SC_RESTORE, SC_SIZE, SC_MOVE,
                        DrawMenuBar,
                        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                    };
                    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

                    if let Ok(hwnd) = main_win.hwnd() {
                        let hwnd_val = hwnd.0 as _;
                        unsafe {
                            let style = GetWindowLongW(hwnd_val, GWL_STYLE);
                            // Completely remove WS_MINIMIZEBOX, WS_MAXIMIZEBOX, and WS_THICKFRAME
                            // so Windows displays ONLY the Close [X] button with no minimize or maximize slots
                            let new_style = style & !(WS_MINIMIZEBOX as i32) & !(WS_MAXIMIZEBOX as i32) & !(WS_THICKFRAME as i32);
                            SetWindowLongW(hwnd_val, GWL_STYLE, new_style);

                            // Clean the system menu as well (remove minimize, maximize, restore, size, and move)
                            let hmenu = GetSystemMenu(hwnd_val, 0);
                            if !hmenu.is_null() {
                                DeleteMenu(hmenu, SC_MINIMIZE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_MAXIMIZE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_RESTORE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_SIZE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_MOVE as u32, MF_BYCOMMAND);
                            }

                            DrawMenuBar(hwnd_val);
                            SetWindowPos(
                                hwnd_val,
                                std::ptr::null_mut(),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                            );

                            // Install Win32 subclass to permanently lock position (non-movable)
                            SetWindowSubclass(hwnd_val, Some(fixed_position_subclass), 1, 0);
                            IS_POSITION_LOCKED.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                }

                // Show landing page upon fresh release install, or if --show, --restarted, or --auto-paste flag passed
                {
                    let args: Vec<String> = std::env::args().collect();
                    let has_pending_paste = args.iter().any(|a| a == "--auto-paste") || {
                        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                            std::path::PathBuf::from(local_app_data).join("CP").join("pending_paste.flag").exists()
                        } else {
                            false
                        }
                    };

                    let force_show = cfg!(debug_assertions) || has_pending_paste || args.iter().any(|a| {
                        a == "--show" || a == "--restarted" || a == "--update-restart" || a == "--landing" || a == "--first-install"
                    });

                    let should_show = if force_show {
                        true
                    } else if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                        let dir = std::path::PathBuf::from(local_app_data).join("CP");
                        let _ = std::fs::create_dir_all(&dir);
                        let flag_file = dir.join("first_install_shown.flag");
                        if !flag_file.exists() {
                            let _ = std::fs::write(&flag_file, "1");
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if should_show {
                        use tauri::Emitter;
                        let _ = main_win.show();
                        let _ = main_win.set_focus();
                        let _ = main_win.emit("show-console", ());
                    }
                }

                // Hide to system tray instead of destroying background driver when X is clicked
                let win_clone = main_win.clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                });
            }

            // Rich System Tray Menu
            if let Ok(tray_icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")) {
                let title_i = MenuItem::with_id(app, "title", "CP", false, None::<&str>)?;
                let status_i = MenuItem::with_id(app, "status", "● Status: ON (intercepting)", false, None::<&str>)?;
                let sep1 = PredefinedMenuItem::separator(app)?;
                let show_i = MenuItem::with_id(app, "show", "Open CP Console", true, None::<&str>)?;
                let autostart_title = if autostart::is_autostart_registered() {
                    "✓ Auto-Start with Windows"
                } else {
                    "Auto-Start with Windows"
                };
                let autostart_i = MenuItem::with_id(app, "autostart", autostart_title, true, None::<&str>)?;
                let reset_i = MenuItem::with_id(app, "reset", "Reset Transfer Metrics", true, None::<&str>)?;
                let sep2 = PredefinedMenuItem::separator(app)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit CP", true, None::<&str>)?;

                let menu = Menu::with_items(app, &[
                    &title_i,
                    &status_i,
                    &sep1,
                    &show_i,
                    &autostart_i,
                    &reset_i,
                    &sep2,
                    &quit_i,
                ])?;

                let _ = tauri::tray::TrayIconBuilder::with_id("cp-tray")
                    .icon(tray_icon)
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .tooltip("CP (Copy/Paste Engine)")
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "show" => {
                                if let Some(win) = app.get_webview_window("main") {
                                    use tauri::Emitter;
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                    let _ = win.emit("show-console", ());
                                }
                            }
                            "autostart" => {
                                let current = autostart::is_autostart_registered();
                                let next_state = !current;
                                let _ = set_autostart_enabled(next_state);
                            }
                            "reset" => {
                                reset_real_stats();
                                if let Some(win) = app.get_webview_window("main") {
                                    use tauri::Emitter;
                                    let _ = win.emit("stats-reset", ());
                                }
                            }
                            "quit" => {
                                resilience::authorize_explicit_exit();
                                app.exit(0);
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        } = event
                        {
                            if let Some(win) = tray.app_handle().get_webview_window("main") {
                                use tauri::Emitter;
                                let _ = win.show();
                                let _ = win.set_focus();
                                let _ = win.emit("show-console", ());
                            }
                        }
                    })
                    .build(app);
            }

            // Synchronize autostart in Windows Registry (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
            #[cfg(not(debug_assertions))]
            autostart::sync_autostart_on_boot();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            get_clipboard_files,
            execute_copy_transfer,
            pause_transfer,
            cancel_transfer,
            hide_window,
            show_window,
            record_transfer,
            reset_stats,
            run_transfer_benchmark,
            check_update,
            get_app_version,
            download_update_container,
            verify_update_signature,
            write_update_staging_file,
            apply_in_process_update,
            check_and_consume_pending_paste,
            is_autostart_enabled,
            set_autostart_enabled
        ])
        .build(tauri::generate_context!())
        .expect("error while building CP desktop application")
        .run(|_app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    if !resilience::is_exit_authorized() {
                        api.prevent_exit();
                    }
                }
                _ => {}
            }
        });
}

#[cfg(all(test, target_os = "windows", target_env = "gnu"))]
mod msvc_gnu_compat {
    #[export_name = "__security_cookie"]
    pub static mut SECURITY_COOKIE: usize = 0x2B99_2DDF_A232;

    #[export_name = "__security_check_cookie"]
    pub unsafe extern "C" fn security_check_cookie(_cookie: usize) {}

    #[export_name = "??_V@YAXPEAX@Z"]
    pub unsafe extern "C" fn msvc_operator_delete_arr(ptr: *mut std::ffi::c_void) {
        if !ptr.is_null() {
            windows_sys::Win32::System::Memory::HeapFree(
                windows_sys::Win32::System::Memory::GetProcessHeap(),
                0,
                ptr,
            );
        }
    }

    #[export_name = "??3@YAXPEAX_K@Z"]
    pub unsafe extern "C" fn msvc_operator_delete_sized(ptr: *mut std::ffi::c_void, _size: usize) {
        if !ptr.is_null() {
            windows_sys::Win32::System::Memory::HeapFree(
                windows_sys::Win32::System::Memory::GetProcessHeap(),
                0,
                ptr,
            );
        }
    }

    #[export_name = "??_U@YAPEAX_K@Z"]
    pub unsafe extern "C" fn msvc_operator_new_arr(size: usize) -> *mut std::ffi::c_void {
        windows_sys::Win32::System::Memory::HeapAlloc(
            windows_sys::Win32::System::Memory::GetProcessHeap(),
            0,
            size,
        )
    }

    #[export_name = "??2@YAPEAX_KAEBUnothrow_t@std@@@Z"]
    pub unsafe extern "C" fn msvc_operator_new_nothrow(size: usize, _nothrow: *const u8) -> *mut std::ffi::c_void {
        windows_sys::Win32::System::Memory::HeapAlloc(
            windows_sys::Win32::System::Memory::GetProcessHeap(),
            0,
            size,
        )
    }

    #[export_name = "?nothrow@std@@3Unothrow_t@1@B"]
    pub static NOTHROW: u8 = 0;

    #[export_name = "_Init_thread_epoch"]
    pub static mut INIT_THREAD_EPOCH: i32 = 0;

    #[export_name = "_Init_thread_header"]
    pub unsafe extern "C" fn init_thread_header(_val: *mut i32) {}

    #[export_name = "_Init_thread_footer"]
    pub unsafe extern "C" fn init_thread_footer(_val: *mut i32) {}
}
