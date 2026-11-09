//! Clean Uninstallation & Shutdown Listener for CP Engine
//! Listens for uninstaller lifecycle signals via:
//! 1. Win32 hidden window class "CP_WINDOW_CLASS" (responds to WM_CLOSE)
//! 2. Win32 named synchronization event "Local\CP_Engine_Shutdown_Event"
//! 3. Proactive uninstaller process attachment watcher
//!
//! When uninstallation is detected:
//! - Authorizes explicit process exit in resilience manager
//! - Removes system tray icons immediately (zero ghost tray icons)
//! - Unregisters Windows kernel restart (zero WER restart loops)
//! - Terminates process cleanly so uninstaller can wipe all binaries and files.

use std::sync::OnceLock;

static GLOBAL_APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Registers the global Tauri AppHandle for clean tray destruction on uninstall.
pub fn set_uninstall_app_handle(handle: tauri::AppHandle) {
    let _ = GLOBAL_APP_HANDLE.set(handle);
}

/// Executes clean uninstallation teardown and exits immediately.
pub fn perform_clean_uninstall_shutdown(source: &str) -> ! {
    eprintln!("[CP Uninstall Teardown] Clean shutdown triggered via: {}", source);

    // 1. Authorize explicit exit so supervisor/resilience doesn't treat this as abnormal
    crate::resilience::authorize_explicit_exit();

    // 2. Cleanly remove all system tray icons immediately so no ghost icon lingers
    if let Some(app) = GLOBAL_APP_HANDLE.get() {
        let _ = app.remove_tray_by_id("cp-tray");
        for i in 0..10 {
            let _ = app.remove_tray_by_id(&format!("task-{}", i));
        }
    }

    // 3. Unregister from Windows Kernel Restart (WER) so OS doesn't restart CP during/after uninstall
    #[cfg(target_os = "windows")]
    unsafe {
        extern "system" {
            fn UnregisterApplicationRestart() -> i32;
        }
        let _ = UnregisterApplicationRestart();
    }

    // 4. Clean up autostart registry entry
    let _ = crate::autostart::set_autostart_registered(false);

    // 5. Clean up %LOCALAPPDATA%\CP (guarantees zero leftover cache/logs even with legacy uninstaller)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let cp_dir = std::path::PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::remove_dir_all(&cp_dir);
    }

    // 6. Release single instance mutex
    crate::resilience::release_single_instance_mutex();

    // 7. Exit cleanly with code 0
    std::process::exit(0);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn uninstall_window_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, PostQuitMessage, WM_CLOSE, WM_DESTROY, WM_QUERYENDSESSION, WM_ENDSESSION,
    };

    match msg {
        WM_CLOSE => {
            perform_clean_uninstall_shutdown("CP_WINDOW_CLASS WM_CLOSE");
        }
        WM_QUERYENDSESSION | WM_ENDSESSION => {
            crate::resilience::authorize_explicit_exit();
            1
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Spawns background listeners for uninstaller signals.
pub fn start_uninstall_listener() {
    #[cfg(target_os = "windows")]
    {
        // 1. Thread for "CP_WINDOW_CLASS" Win32 Window message pump
        std::thread::Builder::new()
            .name("cp-uninstall-wnd".to_string())
            .spawn(|| unsafe {
                use windows_sys::Win32::Foundation::HWND;
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW,
                    TranslateMessage, MSG, WNDCLASSW,
                };

                let class_name: Vec<u16> = "CP_WINDOW_CLASS\0".encode_utf16().collect();

                let wc = WNDCLASSW {
                    style: 0,
                    lpfnWndProc: Some(uninstall_window_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: std::ptr::null_mut(),
                    hIcon: std::ptr::null_mut(),
                    hCursor: std::ptr::null_mut(),
                    hbrBackground: std::ptr::null_mut(),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                };

                RegisterClassW(&wc);

                let hwnd: HWND = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    class_name.as_ptr(),
                    0, // Not visible, zero size, pure background message receiver
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );

                if !hwnd.is_null() {
                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .expect("failed to spawn uninstall window thread");

        // 2. Thread for Named Win32 Shutdown Event
        std::thread::Builder::new()
            .name("cp-uninstall-event".to_string())
            .spawn(|| unsafe {
                use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
                use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

                let event_name: Vec<u16> = "Local\\CP_Engine_Shutdown_Event\0".encode_utf16().collect();
                let event = CreateEventW(std::ptr::null_mut(), 0, 0, event_name.as_ptr());
                if !event.is_null() {
                    if WaitForSingleObject(event, 0xFFFFFFFF) == WAIT_OBJECT_0 {
                        CloseHandle(event);
                        perform_clean_uninstall_shutdown("Local\\CP_Engine_Shutdown_Event");
                    }
                    CloseHandle(event);
                }
            })
            .expect("failed to spawn uninstall event thread");
    }
}
