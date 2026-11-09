//! CP Engine — Power & Background Optimization Subsystem
//!
//! Provides dynamic energy scaling and background resilience:
//! - Idle Background Mode: Minimal energy consumption, memory trim, eco QoS
//! - Active Transfer Boost: Full CPU & PCIe burst, prevents sleep/throttle during copy
//! - Process Working Set Trimming: Keeps idle RAM footprint ultralight (~10MB)

use std::sync::atomic::{AtomicBool, Ordering};

static IS_BOOSTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
#[cfg(target_os = "windows")]
const ES_CONTINUOUS: u32 = 0x80000000;
#[cfg(target_os = "windows")]
const ES_AWAYMODE_REQUIRED: u32 = 0x00000040;

#[cfg(target_os = "windows")]
extern "system" {
    fn SetThreadExecutionState(esFlags: u32) -> u32;
    fn SetProcessWorkingSetSize(
        hProcess: windows_sys::Win32::Foundation::HANDLE,
        dwMinimumWorkingSetSize: usize,
        dwMaximumWorkingSetSize: usize,
    ) -> windows_sys::Win32::Foundation::BOOL;
}

/// Boosts execution priority and prevents OS sleep during active file transfers.
/// "If want get more energy n all but if needed"
#[cfg(target_os = "windows")]
pub fn boost_energy_for_transfer() {
    if IS_BOOSTED.swap(true, Ordering::SeqCst) {
        return;
    }

    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS,
        };

        // 1. Elevate process scheduler priority to HIGH for maximum NVMe/PCIe streaming throughput
        let proc = GetCurrentProcess();
        SetPriorityClass(proc, HIGH_PRIORITY_CLASS);

        // 2. Prevent Windows from sleeping or idling during high-speed data stream
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED);

        // 3. Disable Windows Power Throttling (EcoQoS) for maximum hardware frequency
        set_power_throttling_state(proc, false);
    }
}

/// Returns process to ultralight background idle mode.
/// Conserves battery and minimizes system resource footprint.
#[cfg(target_os = "windows")]
pub fn throttle_down_to_eco_idle() {
    if !IS_BOOSTED.swap(false, Ordering::SeqCst) {
        return;
    }

    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass, NORMAL_PRIORITY_CLASS,
        };

        let proc = GetCurrentProcess();

        // 1. Return process scheduler priority to NORMAL
        SetPriorityClass(proc, NORMAL_PRIORITY_CLASS);

        // 2. Reset execution state to allow natural system power savings
        SetThreadExecutionState(ES_CONTINUOUS);

        // 3. Enable Windows Power Throttling (EcoQoS) for idle background efficiency
        set_power_throttling_state(proc, true);

        // 4. Trim working set memory to release unreferenced pages back to OS
        trim_working_set_memory();
    }
}

#[cfg(not(target_os = "windows"))]
pub fn boost_energy_for_transfer() {}

#[cfg(not(target_os = "windows"))]
pub fn throttle_down_to_eco_idle() {}

/// Trims process working set (RAM) when hidden in tray.
#[cfg(target_os = "windows")]
pub fn trim_working_set_memory() {
    unsafe {
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        // Passing usize::MAX for both min and max working set signals Windows to
        // empty the working set to the standby list without terminating pages.
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn trim_working_set_memory() {}

#[cfg(target_os = "windows")]
unsafe fn set_power_throttling_state(proc: windows_sys::Win32::Foundation::HANDLE, enable_eco: bool) {
    #[repr(C)]
    struct PROCESS_POWER_THROTTLING_STATE {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    const PROCESS_POWER_THROTTLING_CURRENT_VERSION: u32 = 1;
    const PROCESS_POWER_THROTTLING_EXECUTION_SPEED: u32 = 0x1;
    const PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION: u32 = 0x4;

    extern "system" {
        fn SetProcessInformation(
            hProcess: windows_sys::Win32::Foundation::HANDLE,
            ProcessInformationClass: i32,
            ProcessInformation: *const std::ffi::c_void,
            ProcessInformationSize: u32,
        ) -> windows_sys::Win32::Foundation::BOOL;
    }

    const PROCESS_POWER_THROTTLING: i32 = 4;

    let throttling_state = if enable_eco {
        PROCESS_POWER_THROTTLING_STATE {
            version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            control_mask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            state_mask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
        }
    } else {
        PROCESS_POWER_THROTTLING_STATE {
            version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            control_mask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            state_mask: 0,
        }
    };

    let _ = SetProcessInformation(
        proc,
        PROCESS_POWER_THROTTLING,
        &throttling_state as *const _ as *const std::ffi::c_void,
        std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_boost_and_throttle_cycle() {
        boost_energy_for_transfer();
        assert!(IS_BOOSTED.load(Ordering::SeqCst));
        throttle_down_to_eco_idle();
        assert!(!IS_BOOSTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_trim_working_set_does_not_panic() {
        trim_working_set_memory();
    }
}
