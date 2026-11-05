pub mod engine;
pub mod hardware;
pub mod ota;
pub mod interceptor;
pub mod autostart;
pub mod resilience;

use hardware::{probe_system_info, record_completed_transfer, reset_real_stats, SystemInfo};
use engine::{run_benchmark, BenchmarkResult, inspect_clipboard_files, ClipboardFileInfo, copy_file_streaming};
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

    let src = PathBuf::from(&source_path);
    if !src.exists() {
        return Err(format!("Source path does not exist: {}", source_path));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| "Invalid source filename".to_string())?
        .to_os_string();

    let dst_dir = PathBuf::from(&destination_dir);
    if !dst_dir.exists() {
        let _ = std::fs::create_dir_all(&dst_dir);
    }
    let dst = dst_dir.join(file_name);

    let app_handle = app.clone();
    let bytes_copied = tokio::task::spawn_blocking(move || {
        let mut last_emit = std::time::Instant::now();

        copy_file_streaming(&src, &dst, |copied, total, speed_gb| {
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
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
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
async fn download_and_install_update(
    app: tauri::AppHandle,
    manifest: ReleaseManifest,
    pending_paste: Option<bool>,
) -> Result<String, String> {
    let target_version = manifest.version.clone();
    let staged_path = ota::download_and_stage_update(&app, &manifest).await?;
    ota::apply_update_and_restart(app, staged_path, target_version, pending_paste.unwrap_or(false))?;
    Ok("Update staging complete. Restarting...".to_string())
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
                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = main_win.set_icon(icon);
                }

                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        GetWindowLongW, SetWindowLongW, SetWindowPos,
                        GWL_STYLE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
                        GetSystemMenu, DeleteMenu, MF_BYCOMMAND,
                        SC_MINIMIZE, SC_MAXIMIZE, SC_RESTORE, SC_SIZE,
                        DrawMenuBar,
                        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                    };
                    if let Ok(hwnd) = main_win.hwnd() {
                        let hwnd_val = hwnd.0 as _;
                        unsafe {
                            let style = GetWindowLongW(hwnd_val, GWL_STYLE);
                            // Completely remove WS_MINIMIZEBOX, WS_MAXIMIZEBOX, and WS_THICKFRAME
                            // so Windows displays ONLY the Close [X] button with no minimize or maximize slots
                            let new_style = style & !(WS_MINIMIZEBOX as i32) & !(WS_MAXIMIZEBOX as i32) & !(WS_THICKFRAME as i32);
                            SetWindowLongW(hwnd_val, GWL_STYLE, new_style);

                            // Clean the system menu as well
                            let hmenu = GetSystemMenu(hwnd_val, 0);
                            if !hmenu.is_null() {
                                DeleteMenu(hmenu, SC_MINIMIZE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_MAXIMIZE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_RESTORE as u32, MF_BYCOMMAND);
                                DeleteMenu(hmenu, SC_SIZE as u32, MF_BYCOMMAND);
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

                    let force_show = has_pending_paste || args.iter().any(|a| {
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
                        let _ = main_win.show();
                        let _ = main_win.set_focus();
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

            // Dev Mode Watchdog: If the development terminal ends, automatically terminate cp.exe
            // so it never lingers or gets stuck in the system tray when no dev environment is running!
            #[cfg(debug_assertions)]
            {
                std::thread::spawn(|| {
                    use std::net::TcpStream;
                    use std::time::Duration;

                    // Allow 5 seconds grace period for Vite dev server to start
                    std::thread::sleep(Duration::from_secs(5));

                    let mut failed_checks = 0;
                    loop {
                        std::thread::sleep(Duration::from_secs(2));
                        // Ping Vite dev server on port 1420
                        match TcpStream::connect_timeout(
                            &std::net::SocketAddr::from(([127, 0, 0, 1], 1420)),
                            Duration::from_millis(800),
                        ) {
                            Ok(_) => {
                                failed_checks = 0;
                            }
                            Err(_) => {
                                failed_checks += 1;
                                // If Vite is unreachable for 2 consecutive checks, development terminal has ended!
                                if failed_checks >= 2 {
                                    resilience::authorize_explicit_exit();
                                    std::process::exit(0);
                                }
                            }
                        }
                    }
                });
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
            download_and_install_update,
            check_and_consume_pending_paste,
            is_autostart_enabled,
            set_autostart_enabled
        ])
        .build(tauri::generate_context!())
        .expect("error while building CP desktop application")
        .run(|_app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    // In release mode: Do not let random events or window destruction kill the background service!
                    // Only exit if user clicked "Quit CP" in tray (which sets authorize_explicit_exit)
                    #[cfg(not(debug_assertions))]
                    {
                        if !resilience::is_exit_authorized() {
                            api.prevent_exit();
                        }
                    }
                    #[cfg(debug_assertions)]
                    {
                        let _ = api;
                    }
                }
                _ => {}
            }
        });
}
