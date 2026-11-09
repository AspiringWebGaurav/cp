// Prevents console window on Windows across ALL build profiles (GPN architecture)
#![windows_subsystem = "windows"]
#![allow(linker_messages)]

fn main() {
    // 1. Initialize mission-critical resilience & crash recovery (WER kernel auto-restart + panic trap)
    cp_lib::resilience::init_resilience();

    // 2. Base App Golden Snapshot & Crash-Loop Self-Healing Rollback Watchdog
    if !cp_lib::resilience::manage_boot_health_and_auto_rollback() {
        return;
    }

    // 3. Win32 Single Instance check with seamless restart resilience
    #[cfg(target_os = "windows")]
    {
        let acquired = cp_lib::resilience::acquire_single_instance();
        if !acquired {
            return;
        }
    }

    cp_lib::run();
}
