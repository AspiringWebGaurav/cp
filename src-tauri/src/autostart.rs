#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "CP";

/// Checks if CP is currently registered to launch on Windows startup in HKCU\...\Run.
#[cfg(target_os = "windows")]
pub fn is_autostart_registered() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_SUBKEY, KEY_READ) {
        if let Ok(raw_cmd) = run_key.get_value::<String, _>(VALUE_NAME) {
            let trimmed = raw_cmd.trim();
            return !trimmed.is_empty();
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
pub fn is_autostart_registered() -> bool {
    false
}

/// Checks if user explicitly disabled autostart via local app preference flag.
pub fn is_autostart_flag_disabled() -> bool {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = std::path::PathBuf::from(local_app_data).join("CP");
        let pref_file = dir.join("autostart_disabled.flag");
        pref_file.exists()
    } else {
        false
    }
}

/// Sets or removes CP Windows startup registry entry.
/// Value Format: `"C:\path\to\cp.exe" --autostart`
#[cfg(target_os = "windows")]
pub fn set_autostart_registered(enable: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    if enable {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to determine current executable path: {}", e))?;

        let exe_str = current_exe.to_string_lossy().to_string();
        if exe_str.is_empty() {
            return Err("Resolved executable path is empty".to_string());
        }

        let cmd_value = format!("\"{}\" --autostart", exe_str);

        // Fast path: if already registered with exact matching command, avoid redundant disk write
        if let Ok(existing_key) = hkcu.open_subkey_with_flags(RUN_SUBKEY, KEY_READ) {
            if let Ok(existing_val) = existing_key.get_value::<String, _>(VALUE_NAME) {
                if existing_val == cmd_value {
                    return Ok(());
                }
            }
        }

        let (run_key, _disp) = hkcu
            .create_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE | KEY_READ)
            .map_err(|e| format!("Failed to open HKCU\\{}: {}", RUN_SUBKEY, e))?;

        run_key
            .set_value(VALUE_NAME, &cmd_value)
            .map_err(|e| format!("Failed to write autostart registry value: {}", e))?;

        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "[CP Autostart] Registered Windows Startup: HKCU\\{}\\{} -> {}",
            RUN_SUBKEY, VALUE_NAME, cmd_value
        );

        Ok(())
    } else {
        if !is_autostart_registered() {
            return Ok(());
        }

        if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE) {
            let _ = run_key.delete_value(VALUE_NAME);
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "[CP Autostart] Removed Windows Startup entry: HKCU\\{}\\{}",
                RUN_SUBKEY, VALUE_NAME
            );
        }

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_autostart_registered(_enable: bool) -> Result<(), String> {
    Ok(())
}

/// Automatically synchronizes the Windows startup registry entry on application boot.
/// 
/// In production releases, CP maintains its startup registration so it is immediately active
/// and mounted in the background system tray after Windows boots or reboots.
/// Uses 0% idle CPU and zero external network traffic, operating 100% locally.
pub fn sync_autostart_on_boot() {
    #[cfg(target_os = "windows")]
    {
        // Don't overwrite the registry with a debug build binary
        if cfg!(debug_assertions) {
            return;
        }

        if let Ok(current_exe) = std::env::current_exe() {
            let lower = current_exe.to_string_lossy().to_lowercase();
            if lower.contains("target\\debug") || lower.contains("target/debug") {
                return;
            }
        }

        let is_disabled = is_autostart_flag_disabled();
        if is_disabled {
            let _ = set_autostart_registered(false);
        } else {
            let _ = set_autostart_registered(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_autostart_cycle() {
        // Use an isolated test key so tests NEVER delete the real production "CP" autostart entry!
        const TEST_VALUE: &str = "CP_Test_AutoStart";
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        if let Ok((run_key, _)) = hkcu.create_subkey_with_flags(RUN_SUBKEY, winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ) {
            let _ = run_key.set_value(TEST_VALUE, &"\"C:\\test.exe\" --autostart");
            let read_back: Result<String, _> = run_key.get_value(TEST_VALUE);
            assert!(read_back.is_ok());
            let _ = run_key.delete_value(TEST_VALUE);
        }
    }
}
