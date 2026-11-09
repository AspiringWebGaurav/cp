pub mod engine;
pub mod hardware;
pub mod ota;
pub mod interceptor;
pub mod autostart;
pub mod resilience;
pub mod power;
pub mod remote_task;
pub mod uninstall_listener;

use hardware::{probe_system_info, record_completed_transfer, reset_real_stats, SystemInfo};
use engine::{run_benchmark, BenchmarkResult, inspect_clipboard_files, ClipboardFileInfo};
use ota::{check_latest_release, ReleaseManifest};
use std::path::PathBuf;
use tauri::Manager;

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
fn get_remote_task_status() -> Option<remote_task::RemoteTaskRecord> {
    remote_task::get_task_status()
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
fn reset_stats(app: tauri::AppHandle) -> SystemInfo {
    reset_real_stats();
    refresh_tray_menu(&app);
    probe_system_info()
}

#[tauri::command]
fn run_transfer_benchmark(size_mb: u64) -> BenchmarkResult {
    let result = run_benchmark(size_mb);
    record_completed_transfer(result.bytes_copied);
    result
}

fn create_task_tray_menu<M: tauri::Manager<R>, R: tauri::Runtime>(
    app: &M,
    task_id: &str,
    file_name: &str,
    pct: f64,
    speed: &str,
    is_paused: bool,
) -> Result<tauri::menu::Menu<R>, tauri::Error> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

    let display_name = if file_name.len() > 32 {
        format!("{}...", &file_name[..29])
    } else {
        file_name.to_string()
    };
    let title_i = MenuItem::with_id(app, format!("task_title_{}", task_id), &display_name, false, None::<&str>)?;

    let status_str = if is_paused {
        format!("{:.1}% • PAUSED", pct)
    } else {
        format!("{:.1}% • {}", pct, speed)
    };
    let status_i = MenuItem::with_id(app, format!("task_status_{}", task_id), &status_str, false, None::<&str>)?;

    let sep = PredefinedMenuItem::separator(app)?;

    let open_i = MenuItem::with_id(app, format!("open_task_{}", task_id), "Open Transfer Window", true, None::<&str>)?;

    let pause_label = if is_paused { "Resume Transfer" } else { "Pause Transfer" };
    let pause_i = MenuItem::with_id(app, format!("pause_task_{}", task_id), pause_label, true, None::<&str>)?;

    let cancel_i = MenuItem::with_id(app, format!("cancel_task_{}", task_id), "Cancel Transfer", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &title_i,
        &status_i,
        &sep,
        &open_i,
        &pause_i,
        &cancel_i,
    ])?;

    Ok(menu)
}

pub fn format_task_tray_tooltip(pct: f64, speed: &str, file_name: &str, is_paused: bool) -> String {
    let clean_name = if file_name.len() > 42 {
        format!("{}...", &file_name[..39])
    } else {
        file_name.to_string()
    };
    if is_paused {
        format!("CP: {:.1}% (PAUSED)\n{}", pct, clean_name)
    } else {
        format!("CP: {:.1}% ({})\n{}", pct, speed, clean_name)
    }
}

pub fn restore_and_focus_window(win: &tauri::WebviewWindow) {
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();

    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            ShowWindow, SetForegroundWindow, BringWindowToTop, SetWindowPos,
            GetForegroundWindow, GetWindowThreadProcessId,
            SW_RESTORE, SW_SHOW, HWND_TOPMOST, HWND_NOTOPMOST,
            SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentThreadId, AttachThreadInput};

        if let Ok(hwnd) = win.hwnd() {
            let hwnd_val = hwnd.0 as _;
            
            // 1. Force restore & show
            ShowWindow(hwnd_val, SW_RESTORE);
            ShowWindow(hwnd_val, SW_SHOW);

            // 2. Bypass Windows Foreground Lockout via thread attachment
            let fore_hwnd = GetForegroundWindow();
            let fore_thread = GetWindowThreadProcessId(fore_hwnd, std::ptr::null_mut());
            let current_thread = GetCurrentThreadId();

            if fore_thread != 0 && fore_thread != current_thread {
                AttachThreadInput(current_thread, fore_thread, 1);
                BringWindowToTop(hwnd_val);
                SetForegroundWindow(hwnd_val);
                AttachThreadInput(current_thread, fore_thread, 0);
            } else {
                BringWindowToTop(hwnd_val);
                SetForegroundWindow(hwnd_val);
            }

            // 3. Briefly elevate to TOPMOST then NOTOPMOST to guarantee DWM compositor raises the window
            SetWindowPos(hwnd_val, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            SetWindowPos(hwnd_val, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
        }
    }
}

pub fn create_task_tray_icon(
    app: &tauri::AppHandle,
    task_id: &str,
    file_name: &str,
) {
    let tray_id = format!("task-{}", task_id);
    if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32-transfer.png")) {
        let tooltip = format_task_tray_tooltip(0.0, "Starting...", file_name, false);
        let task_id_click = task_id.to_string();
        let task_id_menu = task_id.to_string();

        let menu = match create_task_tray_menu(app, task_id, file_name, 0.0, "Starting...", false) {
            Ok(m) => m,
            Err(_) => return,
        };

        let _ = tauri::tray::TrayIconBuilder::with_id(&tray_id)
            .icon(icon)
            .tooltip(&tooltip)
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_tray_icon_event(move |tray, event| {
                match event {
                    tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    }
                    | tauri::tray::TrayIconEvent::DoubleClick {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } => {
                        let target_win = tray.app_handle().get_webview_window("transfer")
                            .or_else(|| tray.app_handle().get_webview_window("main"));
                        if let Some(win) = target_win {
                            use tauri::Emitter;
                            restore_and_focus_window(&win);
                            let _ = win.emit("focus-task", &task_id_click);
                        }
                    }
                    _ => {}
                }
            })
            .on_menu_event(move |app, event| {
                let event_id = event.id.as_ref();
                if event_id.starts_with("open_task_") {
                    let target_win = app.get_webview_window("transfer")
                        .or_else(|| app.get_webview_window("main"));
                    if let Some(win) = target_win {
                        use tauri::Emitter;
                        restore_and_focus_window(&win);
                        let _ = win.emit("focus-task", &task_id_menu);
                    }
                } else if event_id.starts_with("pause_task_") {
                    if let Some(task) = engine::get_active_task(&task_id_menu) {
                        let next_paused = !task.is_paused.load(std::sync::atomic::Ordering::SeqCst);
                        engine::set_task_paused(Some(&task_id_menu), next_paused);
                        let target_win = app.get_webview_window("transfer")
                            .or_else(|| app.get_webview_window("main"));
                        if let Some(win) = target_win {
                            use tauri::Emitter;
                            let _ = win.emit("task-paused-changed", serde_json::json!({
                                "taskId": &task_id_menu,
                                "isPaused": next_paused
                            }));
                        }
                    }
                } else if event_id.starts_with("cancel_task_") {
                    engine::cancel_task_by_id(Some(&task_id_menu));
                    let tray_id = format!("task-{}", &task_id_menu);
                    let _ = app.remove_tray_by_id(&tray_id);
                    let target_win = app.get_webview_window("transfer")
                        .or_else(|| app.get_webview_window("main"));
                    if let Some(win) = target_win {
                        use tauri::Emitter;
                        let _ = win.emit("task-cancelled", &task_id_menu);
                    }
                }
            })
            .build(app);
    }
}

pub fn update_task_tray_tooltip<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    task_id: &str,
    file_name: &str,
    pct: f64,
    speed: &str,
    is_paused: bool,
) {
    let tray_id = format!("task-{}", task_id);
    if let Some(tray) = app.tray_by_id(&tray_id) {
        let tooltip = format_task_tray_tooltip(pct, speed, file_name, is_paused);
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

#[tauri::command]
fn check_file_conflicts(
    file_paths: Vec<String>,
    destination_dir: String,
    is_cut: Option<bool>,
) -> Vec<engine::FileConflictInfo> {
    engine::check_file_conflicts(&file_paths, &destination_dir, is_cut.unwrap_or(false))
}

#[tauri::command]
async fn execute_copy_transfer(
    app: tauri::AppHandle,
    source_path: String,
    destination_dir: String,
    is_cut: Option<bool>,
    task_id: Option<String>,
    conflict_action: Option<String>,
) -> Result<SystemInfo, String> {
    use tauri::Emitter;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    let is_move = is_cut.unwrap_or(false);
    engine::reset_transfer_cancel();

    let id = task_id.unwrap_or_else(|| {
        format!("task-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis())
    });

    let src = PathBuf::from(&source_path);
    if !src.exists() {
        return Err(format!("Source path does not exist: {}", source_path));
    }

    let dst_dir = engine::normalize_destination_dir(&destination_dir);
    if !dst_dir.exists() {
        let _ = std::fs::create_dir_all(&dst_dir);
    }

    let file_name = match src.file_name() {
        Some(name) => name.to_os_string(),
        None => return Err("Invalid source file name".to_string()),
    };
    let file_name_str = file_name.to_string_lossy().to_string();

    let total_file_bytes = if src.is_dir() {
        engine::calculate_path_size(&src)
    } else {
        std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0)
    };

    let task = Arc::new(engine::ActiveTask {
        id: id.clone(),
        source: source_path.clone(),
        destination: destination_dir.clone(),
        file_name: file_name_str.clone(),
        is_cut: is_move,
        is_cancelled: Arc::new(AtomicBool::new(false)),
        is_paused: Arc::new(AtomicBool::new(false)),
        current_dest_path: Arc::new(Mutex::new(None)),
        bytes_copied: Arc::new(AtomicU64::new(0)),
        total_bytes: Arc::new(AtomicU64::new(total_file_bytes)),
        progress_pct: Arc::new(Mutex::new(0.0)),
        speed_str: Arc::new(Mutex::new("Starting...".to_string())),
    });

    engine::register_active_task(task.clone());
    power::boost_energy_for_transfer();

    // Create dynamic system tray icon for this task
    create_task_tray_icon(&app, &id, &file_name_str);

    struct TaskGuard {
        app: tauri::AppHandle,
        task_id: String,
    }
    impl Drop for TaskGuard {
        fn drop(&mut self) {
            let tray_id = format!("task-{}", self.task_id);
            let _ = self.app.remove_tray_by_id(&tray_id);
            engine::unregister_active_task(&self.task_id);
            if engine::count_active_tasks() == 0 {
                engine::release_transfer();
                power::throttle_down_to_eco_idle();
                power::trim_working_set_memory();
            }
        }
    }
    let _guard = TaskGuard {
        app: app.clone(),
        task_id: id.clone(),
    };

    // If moving within the exact same directory, it's an immediate no-op
    if is_move && src.parent() == Some(&dst_dir) {
        return Ok(probe_system_info());
    }

    let action = conflict_action.as_deref().unwrap_or("keep_both");
    if action == "skip" {
        return Ok(probe_system_info());
    }

    let dst = if action == "replace" {
        dst_dir.join(&file_name)
    } else if is_move && !dst_dir.join(&file_name).exists() {
        dst_dir.join(&file_name)
    } else {
        engine::generate_unique_destination_path(&src, &dst_dir)
    };

    let part_dst = dst.with_file_name(format!("{}.cp.part", file_name_str));
    engine::begin_transaction_transfer(&id, &part_dst, &dst);

    // Fast-path: If moving on the same drive volume, try instantaneous atomic rename!
    if is_move {
        if let Ok(()) = std::fs::rename(&src, &dst) {
            let total = if dst.is_dir() {
                engine::calculate_path_size(&dst)
            } else {
                std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0)
            };
            let update = engine::ProgressUpdate {
                task_id: Some(id.clone()),
                bytes_copied: total,
                total_bytes: total,
                formatted_copied: crate::hardware::format_bytes(total),
                formatted_total: crate::hardware::format_bytes(total),
                percentage: 100.0,
                speed: "5.00 GB/s".to_string(),
                eta: "Complete!".to_string(),
                is_finished: true,
            };
            let _ = app.emit("transfer-progress", update);

            let _ = engine::commit_active_transaction(&id, &dst, is_move);
            record_completed_transfer(total);
            refresh_tray_menu(&app);
            return Ok(probe_system_info());
        }
    }

    let app_handle = app.clone();
    let task_for_streaming = task.clone();
    let id_for_progress = id.clone();
    let fname_for_progress = file_name_str.clone();
    let dst_for_streaming = dst.clone();

    let bytes_copied = tokio::task::spawn_blocking(move || {
        let mut last_emit = std::time::Instant::now();
        let mut last_tray_update = std::time::Instant::now();

        engine::copy_path_streaming_for_task(&task_for_streaming, &src, &dst_for_streaming, |copied, total, speed_gb| {
            let now = std::time::Instant::now();
            let is_finished = copied == total;
            let should_emit = is_finished || now.duration_since(last_emit).as_millis() >= 40;

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

                let rounded_pct = (pct * 10.0).round() / 10.0;
                let is_paused = task_for_streaming.is_paused.load(Ordering::SeqCst);

                if let Ok(mut p) = task_for_streaming.progress_pct.lock() {
                    *p = rounded_pct;
                }
                if let Ok(mut s) = task_for_streaming.speed_str.lock() {
                    *s = speed_str.clone();
                }

                // Throttle tray tooltip updates to once every 800ms or when finished
                // This prevents Win32 notification queue choking and keeps tray clicks instantly responsive
                let should_tray_update = is_finished || now.duration_since(last_tray_update).as_millis() >= 800;
                if should_tray_update {
                    last_tray_update = now;
                    update_task_tray_tooltip(
                        &app_handle,
                        &id_for_progress,
                        &fname_for_progress,
                        rounded_pct,
                        &speed_str,
                        is_paused,
                    );
                }

                let update = engine::ProgressUpdate {
                    task_id: Some(id_for_progress.clone()),
                    bytes_copied: copied,
                    total_bytes: total,
                    formatted_copied: crate::hardware::format_bytes(copied),
                    formatted_total: crate::hardware::format_bytes(total),
                    percentage: rounded_pct,
                    speed: speed_str,
                    eta: eta_str,
                    is_finished,
                };

                let _ = app_handle.emit("transfer-progress", update);
            }
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    // If it was a Move (Cut), delete source path upon verified destination completion
    if is_move {
        let src_to_delete = PathBuf::from(&source_path);
        engine::robust_delete_path(&src_to_delete);
    }

    engine::commit_active_transaction(&id, &dst, is_move)?;
    record_completed_transfer(bytes_copied);
    refresh_tray_menu(&app);
    Ok(probe_system_info())
}

#[tauri::command]
fn pause_transfer(paused: bool, task_id: Option<String>) {
    engine::set_task_paused(task_id.as_deref(), paused);
}

#[tauri::command]
fn cancel_transfer(app: tauri::AppHandle, task_id: Option<String>) {
    if let Some(id) = task_id.as_deref() {
        let tray_id = format!("task-{}", id);
        let _ = app.remove_tray_by_id(&tray_id);
    }
    engine::cancel_active_transaction(task_id.as_deref());
}

#[tauri::command]
fn show_transfer_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("transfer") {
        set_window_movable(true);
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 600.0,
            height: 315.0,
        }));
        restore_and_focus_window(&win);
    } else if let Some(win) = app.get_webview_window("main") {
        set_window_movable(true);
        let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: 600.0,
            height: 315.0,
        }));
        restore_and_focus_window(&win);
    }
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("transfer") {
        let _ = win.hide();
    } else if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    power::trim_working_set_memory();
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("transfer") {
        let _ = win.hide();
    } else if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    power::trim_working_set_memory();
    refresh_tray_menu(&app);
}

#[tauri::command]
fn trim_memory() {
    power::trim_working_set_memory();
}

fn create_dynamic_tray_menu<M: tauri::Manager<R>, R: tauri::Runtime>(app: &M) -> Result<tauri::menu::Menu<R>, tauri::Error> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

    let is_hook_on = interceptor::is_interceptor_enabled();

    // 1. Header with App Title
    let header_i = MenuItem::with_id(app, "header", "CP", false, None::<&str>)?;

    // 2. Dynamic Interception Status (Interactive toggle: click to pause / resume)
    let status_text = if is_hook_on {
        "• Status: ON (intercepting)"
    } else {
        "• Status: PAUSED"
    };
    let status_i = MenuItem::with_id(app, "toggle_interceptor", status_text, true, None::<&str>)?;

    let sep = PredefinedMenuItem::separator(app)?;

    // 3. Open CP Console
    let show_i = MenuItem::with_id(app, "show_console", "Open CP Console", true, None::<&str>)?;

    // 4. Check for Updates
    let update_i = MenuItem::with_id(app, "check_update", "Check for Updates", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &header_i,
        &status_i,
        &sep,
        &show_i,
        &update_i,
    ])?;

    Ok(menu)
}

pub fn refresh_tray_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(tray) = app.tray_by_id("cp-tray") {
        if let Ok(menu) = create_dynamic_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

#[tauri::command]
fn is_interceptor_enabled() -> bool {
    interceptor::is_interceptor_enabled()
}

#[tauri::command]
fn toggle_interceptor(app: tauri::AppHandle) -> bool {
    let next = !interceptor::is_interceptor_enabled();
    interceptor::set_interceptor_enabled(next);
    refresh_tray_menu(&app);
    next
}

static SHOULD_SHOW_ON_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static HAS_SHOWN_INITIAL_WINDOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn notify_app_ready(app: tauri::AppHandle, window: tauri::WebviewWindow) {
    use tauri::Manager;
    // Only window "main" (Landing UI) is allowed to be automatically revealed on initial ready
    if window.label() == "main" && SHOULD_SHOW_ON_READY.load(std::sync::atomic::Ordering::SeqCst) {
        if !HAS_SHOWN_INITIAL_WINDOW.swap(true, std::sync::atomic::Ordering::SeqCst) {
            if let Some(win) = app.get_webview_window("main") {
                set_window_movable(false);
                let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                    width: 600.0,
                    height: 206.0,
                }));
                let _ = win.center();
                restore_and_focus_window(&win);
                use tauri::Emitter;
                let _ = win.emit("show-console", ());
            }
        }
    }
}

#[tauri::command]
fn show_window(app: tauri::AppHandle) {
    use tauri::Manager;
    if let Some(win) = app.get_webview_window("transfer") {
        restore_and_focus_window(&win);
    } else if let Some(win) = app.get_webview_window("main") {
        restore_and_focus_window(&win);
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
async fn execute_auto_update(
    app: tauri::AppHandle,
    url: String,
    target_version: String,
    signature: Option<String>,
    active_tab: Option<String>,
    pending_paste: Option<bool>,
) -> Result<(), String> {
    let staged_path = ota::download_verify_and_stage_update(
        &app,
        &url,
        signature.as_deref(),
        &target_version,
    )
    .await?;

    ota::apply_update_and_restart(
        app,
        staged_path,
        target_version,
        active_tab,
        pending_paste.unwrap_or(false),
    )
}

#[tauri::command]
fn simulate_release_broadcast(app: tauri::AppHandle, manifest: ReleaseManifest) -> Result<(), String> {
    use tauri::Emitter;
    app.emit("ota-release-broadcast", &manifest).map_err(|e| e.to_string())
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

        // Purge any stale / obsolete staged binaries or leftover .part files from previous updates
        if let Ok(entries) = std::fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p != target_file && p != part_file {
                    let _ = std::fs::remove_file(p);
                }
            }
        }

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RestoredLifecycleResponse {
    pub was_updated: bool,
    pub previous_version: Option<String>,
    pub current_version: String,
    pub restored_tab: String,
    pub has_pending_paste: bool,
    pub clipboard_files: Option<ClipboardFileInfo>,
    #[serde(default)]
    pub is_base_reset: bool,
}

#[tauri::command]
fn restore_lifecycle_state() -> RestoredLifecycleResponse {
    let current_ver = ota::resolve_current_version();
    let lifecycle_opt = ota::load_lifecycle_state();
    let restored_clip = engine::load_and_restore_saved_clipboard();

    let mut was_updated = false;
    let mut prev_ver = None;
    let mut restored_tab = "status".to_string();
    let mut has_pending_paste = false;
    let mut is_base_reset = false;

    if let Some(mut lc) = lifecycle_opt {
        if lc.state == "RESTARTING" || lc.state == "INSTALLING" || lc.state == "RESET_TO_BASE" || (lc.state == "COMPLETED" && lc.attempt_count > 0) {
            was_updated = true;
            if lc.state == "RESET_TO_BASE" {
                is_base_reset = true;
            }
            prev_ver = Some(lc.previous_version.clone());
            restored_tab = lc.active_tab.clone();
            has_pending_paste = lc.pending_paste;

            // Mark lifecycle as consumed
            lc.state = "CONSUMED".to_string();
            lc.attempt_count = 0;
            lc.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            ota::persist_lifecycle_state(&lc);
        }
    }

    // Also check pending paste flag
    if check_and_consume_pending_paste() {
        has_pending_paste = true;
    }

    RestoredLifecycleResponse {
        was_updated,
        previous_version: prev_ver,
        current_version: current_ver,
        restored_tab,
        has_pending_paste,
        clipboard_files: restored_clip,
        is_base_reset,
    }
}

#[tauri::command]
fn get_update_lifecycle_state() -> Option<ota::UpdateLifecycleState> {
    ota::load_lifecycle_state()
}

#[tauri::command]
fn quarantine_version_ipc(version: String) {
    ota::quarantine_version(&version);
}

#[tauri::command]
fn apply_in_process_update(
    app: tauri::AppHandle,
    staging_path: String,
    target_version: String,
    active_tab: Option<String>,
    pending_paste: Option<bool>,
) -> Result<(), String> {
    ota::apply_update_and_restart(
        app,
        std::path::PathBuf::from(staging_path),
        target_version,
        active_tab,
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

static IS_WINDOW_MOVABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_window_movable(movable: bool) {
    IS_WINDOW_MOVABLE.store(movable, std::sync::atomic::Ordering::SeqCst);
}

pub fn is_window_movable() -> bool {
    IS_WINDOW_MOVABLE.load(std::sync::atomic::Ordering::SeqCst)
}

#[tauri::command]
fn open_destination_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let clean_path = path.trim().replace('/', "\\");
        let p = std::path::Path::new(&clean_path);
        let target_dir = if p.is_file() {
            p.parent().unwrap_or(p).to_string_lossy().to_string()
        } else {
            clean_path
        };

        let wide_path: Vec<u16> = OsStr::new(&target_dir).encode_wide().chain(std::iter::once(0)).collect();
        let wide_open: Vec<u16> = OsStr::new("open").encode_wide().chain(std::iter::once(0)).collect();

        unsafe {
            let res = ShellExecuteW(
                std::ptr::null_mut(),
                wide_open.as_ptr(),
                wide_path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL as i32,
            );
            if (res as isize) <= 32 {
                let _ = std::process::Command::new("explorer")
                    .arg(&target_dir)
                    .spawn();
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Ok(())
    }
}

#[tauri::command]
fn set_ui_mode(app: tauri::AppHandle, mode: String) {
    use tauri::Manager;
    if mode == "landing" || mode == "status" {
        if let Some(win) = app.get_webview_window("main") {
            let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 600.0,
                height: 206.0,
            }));
            let _ = win.center();
            set_window_movable(false);
        }
    } else {
        if let Some(win) = app.get_webview_window("transfer") {
            set_window_movable(true);
            let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 600.0,
                height: 315.0,
            }));
            if mode == "conflict" {
                let _ = win.center();
            }
        } else if let Some(win) = app.get_webview_window("main") {
            set_window_movable(true);
            let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 600.0,
                height: 315.0,
            }));
            if mode == "conflict" {
                let _ = win.center();
            }
        }
    }
}

#[tauri::command]
fn execute_remote_task_action(action: String, payload: Option<String>) -> Result<String, String> {
    let payload_val = payload.and_then(|p| serde_json::from_str(&p).ok());
    let directive = remote_task::RemoteTaskDirective {
        task_id: Some(format!("ipc-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())),
        action: Some(action),
        target_version: None,
        payload: payload_val,
    };
    remote_task::execute_remote_directive(&directive)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn window_tray_subclass(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
    uid_subclass: usize,
    _ref_data: usize,
) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_SYSCOMMAND, SC_MINIMIZE, SC_MOVE, SW_HIDE, ShowWindow,
        WM_NCHITTEST, HTCAPTION, HTBORDER,
        WM_WINDOWPOSCHANGING, WINDOWPOS, SWP_NOMOVE,
        WM_QUERYENDSESSION, WM_ENDSESSION,
    };
    use windows_sys::Win32::UI::Shell::DefSubclassProc;

    // 0. Intercept Windows session end / RestartManager shutdown: authorize exit cleanly
    if msg == WM_QUERYENDSESSION || msg == WM_ENDSESSION {
        crate::resilience::authorize_explicit_exit();
        return 1;
    }

    // 1. Intercept SC_MINIMIZE: smoothly hide window to system tray instead of leaving taskbar clutter
    if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_MINIMIZE as usize {
        ShowWindow(hwnd, SW_HIDE);
        return 0;
    }

    // 2. Lock Landing UI (subclass 1) in center vs Allow Copy/Paste (subclass 2) to be freely movable
    if uid_subclass == 1 && !is_window_movable() {
        if msg == WM_NCHITTEST {
            let hit = DefSubclassProc(hwnd, msg, wparam, lparam);
            if hit == HTCAPTION as isize {
                return HTBORDER as isize;
            }
            return hit;
        }

        if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_MOVE as usize {
            return 0;
        }

        if msg == WM_WINDOWPOSCHANGING {
            let pos = &mut *(lparam as *mut WINDOWPOS);
            pos.flags |= SWP_NOMOVE;
            return 0;
        }
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;

            // Start uninstaller lifecycle listener & attach global app handle for clean tray wipe
            uninstall_listener::set_uninstall_app_handle(app.handle().clone());
            uninstall_listener::start_uninstall_listener();

            // Start real-time push update stream listener (sub-second edge release notifications)
            ota::start_release_stream_listener(app.handle().clone());

            // Sweep and clean any orphaned/in-flight partial transfers from prior crashed runs
            engine::cleanup_all_in_flight_transfers();

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
                        SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, SC_SIZE,
                        DrawMenuBar,
                        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                    };
                    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

                    if let Ok(hwnd) = main_win.hwnd() {
                        let hwnd_val = hwnd.0 as _;
                        unsafe {
                            let style = GetWindowLongW(hwnd_val, GWL_STYLE);
                            // Remove WS_MINIMIZEBOX and WS_MAXIMIZEBOX so Windows displays ONLY the [X] close button (clean reader modal)
                            let new_style = style & !(WS_MINIMIZEBOX as i32) & !(WS_MAXIMIZEBOX as i32) & !(WS_THICKFRAME as i32);
                            SetWindowLongW(hwnd_val, GWL_STYLE, new_style);

                            // Clean the system menu: remove SC_MINIMIZE, SC_MAXIMIZE, SC_RESTORE, and SC_SIZE
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

                            // Install Win32 subclass to smoothly intercept minimize to system tray while allowing free dragging
                            SetWindowSubclass(hwnd_val, Some(window_tray_subclass), 1, 0);
                        }
                    }
                }

                // Execute remote verification task on startup / fresh restart
                remote_task::execute_remote_task();

                // Hide to system tray instead of destroying background driver when X is clicked
                let win_clone = main_win.clone();
                let app_handle_clone = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if crate::resilience::is_exit_authorized() {
                            return;
                        }
                        api.prevent_close();
                        let _ = win_clone.hide();
                        power::trim_working_set_memory();
                        refresh_tray_menu(&app_handle_clone);
                    }
                });
            }

            if let Some(transfer_win) = app.get_webview_window("transfer") {
                let _ = transfer_win.center();

                if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = transfer_win.set_icon(icon);
                }

                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        GetWindowLongW, SetWindowLongW, SetWindowPos,
                        GWL_STYLE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
                        GetSystemMenu, DeleteMenu, MF_BYCOMMAND,
                        SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, SC_SIZE,
                        DrawMenuBar,
                        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                    };
                    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

                    if let Ok(hwnd) = transfer_win.hwnd() {
                        let hwnd_val = hwnd.0 as _;
                        unsafe {
                            let style = GetWindowLongW(hwnd_val, GWL_STYLE);
                            let new_style = style & !(WS_MINIMIZEBOX as i32) & !(WS_MAXIMIZEBOX as i32) & !(WS_THICKFRAME as i32);
                            SetWindowLongW(hwnd_val, GWL_STYLE, new_style);

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

                            // Subclass with UID 2: allows free dragging and movement everywhere on screen
                            SetWindowSubclass(hwnd_val, Some(window_tray_subclass), 2, 0);
                        }
                    }
                }

                // Hide to system tray and cancel in-flight transfer when X is clicked
                let transfer_clone = transfer_win.clone();
                let app_handle_clone = app.handle().clone();
                transfer_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if crate::resilience::is_exit_authorized() {
                            return;
                        }
                        api.prevent_close();
                        let _ = transfer_clone.hide();
                        engine::cancel_active_transaction(None);
                        power::trim_working_set_memory();
                        refresh_tray_menu(&app_handle_clone);
                    }
                });
            }

            // Meaningful & Dynamic System Tray Menu (No Exit Option)
            if let Ok(tray_icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png")) {
                let menu = create_dynamic_tray_menu(app)?;

                let _ = tauri::tray::TrayIconBuilder::with_id("cp-tray")
                    .icon(tray_icon)
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .tooltip("CP (Copy/Paste Engine)")
                    .on_menu_event(|app, event| {
                        match event.id.as_ref() {
                            "show_console" => {
                                if let Some(win) = app.get_webview_window("main") {
                                    use tauri::Emitter;
                                    set_window_movable(false);
                                    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                                        width: 600.0,
                                        height: 206.0,
                                    }));
                                    let _ = win.center();
                                    restore_and_focus_window(&win);
                                    let _ = win.emit("show-console", ());
                                }
                            }
                            "toggle_interceptor" => {
                                let current = interceptor::is_interceptor_enabled();
                                interceptor::set_interceptor_enabled(!current);
                                refresh_tray_menu(app);
                                if let Some(win) = app.get_webview_window("main") {
                                    use tauri::Emitter;
                                    let _ = win.emit("stats-reset", ());
                                }
                            }
                            "check_update" => {
                                if let Some(win) = app.get_webview_window("main") {
                                    use tauri::Emitter;
                                    restore_and_focus_window(&win);
                                    let _ = win.emit("trigger-update-check", ());
                                }
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event| {
                        match event {
                            tauri::tray::TrayIconEvent::Click {
                                button: tauri::tray::MouseButton::Left,
                                button_state: tauri::tray::MouseButtonState::Up,
                                ..
                            }
                            | tauri::tray::TrayIconEvent::DoubleClick {
                                button: tauri::tray::MouseButton::Left,
                                ..
                            } => {
                                if let Some(win) = tray.app_handle().get_webview_window("main") {
                                    use tauri::Emitter;
                                    set_window_movable(false);
                                    let _ = win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                                        width: 600.0,
                                        height: 206.0,
                                    }));
                                    let _ = win.center();
                                    restore_and_focus_window(&win);
                                    let _ = win.emit("show-console", ());
                                }
                            }
                            _ => {}
                        }
                    })
                    .build(app);
            }

            // =========================================================================
            // Post-Setup Window Presentation & One-Time First-Install Landing UI
            // "after install all system tray n all setting up instant show landing ui to user just one time"
            // =========================================================================
            if let Some(main_win) = app.get_webview_window("main") {
                let args: Vec<String> = std::env::args().collect();
                let has_pending_paste = args.iter().any(|a| a == "--auto-paste") || {
                    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                        let flag = std::path::PathBuf::from(&local_app_data).join("CP").join("pending_paste.flag");
                        if flag.exists() {
                            let _ = std::fs::remove_file(&flag);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                // One-time first-install trigger:
                // Triggers if "--first-install" or "--first-run" is passed,
                // OR if the .first_run_completed flag does not exist in %LOCALAPPDATA%\CP
                let is_first_install_flag = args.iter().any(|a| a == "--first-install" || a == "--first-run");
                let is_one_time_first_install = {
                    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                        let cp_dir = std::path::PathBuf::from(&local_app_data).join("CP");
                        let marker_path = cp_dir.join(".first_run_completed");
                        if is_first_install_flag || !marker_path.exists() {
                            let _ = std::fs::create_dir_all(&cp_dir);
                            let _ = std::fs::write(&marker_path, b"1");
                            true
                        } else {
                            false
                        }
                    } else {
                        is_first_install_flag
                    }
                };

                let is_explicit_show = args.iter().any(|a| a == "--show" || a == "--landing");
                let is_update_restart = args.iter().any(|a| a == "--restarted" || a == "--update-restart");

                // Silent Tray & Dual Entity Architecture:
                // In terminal / npm run dev / background autostart: Keep window strictly hidden in system tray!
                // Window opens strictly on explicit user action (e.g. clicking the tray icon),
                // or just one time on first-install / after-install setup,
                // or when an automated update restart finishes and reopens the appropriate entity.
                if has_pending_paste {
                    // Separate Copy/Paste Entity: Restart directly into Transfer View!
                    if let Some(transfer_win) = app.get_webview_window("transfer") {
                        set_window_movable(true);
                        let _ = transfer_win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                            width: 600.0,
                            height: 315.0,
                        }));
                        let _ = transfer_win.center();
                        restore_and_focus_window(&transfer_win);
                    } else {
                        set_window_movable(true);
                        let _ = main_win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                            width: 600.0,
                            height: 315.0,
                        }));
                        let _ = main_win.center();
                        restore_and_focus_window(&main_win);
                    }
                    // Do NOT emit "show-console" — keeps Landing UI completely separate!
                } else if is_one_time_first_install || is_explicit_show || is_update_restart {
                    // Mark flag so that when React finishes mounting and painting the DOM,
                    // notify_app_ready instantly reveals the window with ZERO blank/white flash!
                    SHOULD_SHOW_ON_READY.store(true, std::sync::atomic::Ordering::SeqCst);

                    // Robust safety fallback: if notify_app_ready is ever delayed, reveal after 400ms
                    let fallback_win = main_win.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                        if SHOULD_SHOW_ON_READY.load(std::sync::atomic::Ordering::SeqCst)
                            && !HAS_SHOWN_INITIAL_WINDOW.swap(true, std::sync::atomic::Ordering::SeqCst)
                        {
                            set_window_movable(false);
                            let _ = fallback_win.set_size(tauri::Size::Logical(tauri::LogicalSize {
                                width: 600.0,
                                height: 206.0,
                            }));
                            let _ = fallback_win.center();
                            restore_and_focus_window(&fallback_win);
                            use tauri::Emitter;
                            let _ = fallback_win.emit("show-console", ());
                        }
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            notify_app_ready,
            get_system_info,
            get_clipboard_files,
            check_file_conflicts,
            execute_copy_transfer,
            pause_transfer,
            cancel_transfer,
            minimize_window,
            hide_window,
            show_window,
            record_transfer,
            reset_stats,
            run_transfer_benchmark,
            check_update,
            get_app_version,
            execute_auto_update,
            simulate_release_broadcast,
            download_update_container,
            verify_update_signature,
            write_update_staging_file,
            apply_in_process_update,
            restore_lifecycle_state,
            get_update_lifecycle_state,
            quarantine_version_ipc,
            check_and_consume_pending_paste,
            is_autostart_enabled,
            set_autostart_enabled,
            is_interceptor_enabled,
            toggle_interceptor,
            set_ui_mode,
            trim_memory,
            open_destination_folder,
            show_transfer_window,
            get_remote_task_status,
            execute_remote_task_action
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
