// Prevents console window on Windows across ALL build profiles (GPN architecture)
#![windows_subsystem = "windows"]

fn main() {
    // 1. Initialize mission-critical resilience & crash recovery (WER kernel auto-restart + panic trap)
    cp_lib::resilience::init_resilience();

    // 2. Win32 Single Instance check with seamless restart resilience
    #[cfg(target_os = "windows")]
    {
        if !cp_lib::resilience::acquire_single_instance() {
            return;
        }
    }

    cp_lib::run();
}
