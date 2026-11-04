use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub fn start_explorer_interceptor(app: AppHandle) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL,
    };

    let _ = APP_HANDLE.set(app);
    if HOOK_ACTIVE.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(|| unsafe {
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            std::ptr::null_mut(),
            0,
        );

        if hook.is_null() {
            return;
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(hook);
    });
}

#[cfg(not(target_os = "windows"))]
pub fn start_explorer_interceptor(_app: AppHandle) {}

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::System::DataExchange::IsClipboardFormatAvailable;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetClassNameW, GetForegroundWindow, KBDLLHOOKSTRUCT, WM_KEYDOWN,
    };

    if code >= 0 && (wparam == WM_KEYDOWN as usize) {
        let kbd = *(lparam as *const KBDLLHOOKSTRUCT);
        // VK_V is 0x56
        if kbd.vkCode == 0x56 {
            // Check if Ctrl is held down: VK_CONTROL (0x11)
            let ctrl_down = (GetAsyncKeyState(0x11) as u16 & 0x8000) != 0;
            if ctrl_down {
                // Interrogate foreground window
                let hwnd = GetForegroundWindow();
                if !hwnd.is_null() {
                    let mut class_buf = [0u16; 256];
                    let len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
                    let class_name = String::from_utf16_lossy(&class_buf[..len.max(0) as usize]);

                    let is_explorer = class_name == "CabinetWClass"
                        || class_name == "ExploreWClass"
                        || class_name == "Progman"
                        || class_name == "WorkerW";

                    if is_explorer {
                        // Check if clipboard contains file drop list: CF_HDROP (15)
                        const CF_HDROP: u32 = 15;
                        if IsClipboardFormatAvailable(CF_HDROP) != 0 {
                            // SUPPRESS Windows Explorer's built-in paste!
                            // Trigger CP's high-speed copy transfer immediately!
                            if let Some(app) = APP_HANDLE.get() {
                                if let Some(win) = app.get_webview_window("main") {
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                    let _ = win.emit("trigger-auto-paste", ());
                                }
                            }
                            // Return 1 to cancel the keystroke in Windows Explorer
                            // This guarantees Windows' slow copy dialog NEVER appears!
                            return 1;
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

