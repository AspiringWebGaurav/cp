// Prevents console window on Windows across ALL build profiles (GPN architecture)
#![windows_subsystem = "windows"]
#![allow(linker_messages)]

// Provide MSVC CRT buffer security cookies & C++ runtime symbols for MinGW (GNU) linker when linking WebView2LoaderStatic
#[cfg(all(target_os = "windows", target_env = "gnu"))]
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

fn main() {
    // 1. Initialize mission-critical resilience & crash recovery (WER kernel auto-restart + panic trap)
    cp_lib::resilience::init_resilience();

    // 2. Win32 Single Instance check with seamless restart resilience
    #[cfg(target_os = "windows")]
    {
        let acquired = cp_lib::resilience::acquire_single_instance();
        if !acquired {
            return;
        }
    }

    cp_lib::run();
}
