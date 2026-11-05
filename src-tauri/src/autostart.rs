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

        let (run_key, _disp) = hkcu
            .create_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE | KEY_READ)
            .map_err(|e| format!("Failed to open HKCU\\{}: {}", RUN_SUBKEY, e))?;

        run_key
            .set_value(VALUE_NAME, &cmd_value)
            .map_err(|e| format!("Failed to write autostart registry value: {}", e))?;

        eprintln!(
            "[CP Autostart] Registered Windows Startup: HKCU\\{}\\{} -> {}",
            RUN_SUBKEY, VALUE_NAME, cmd_value
        );

        Ok(())
    } else {
        if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_SUBKEY, KEY_SET_VALUE) {
            let _ = run_key.delete_value(VALUE_NAME);
            eprintln!(
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
/// in the background system tray after Windows boots or reboots.
/// Uses 0% idle CPU and zero external network traffic, operating 100% locally.
pub fn sync_autostart_on_boot() {
    #[cfg(target_os = "windows")]
    {
        // Check if user has explicitly disabled autostart in LocalAppData setting
        let mut is_disabled = false;
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let pref_file = std::path::PathBuf::from(local_app_data)
                .join("CP")
                .join("autostart_disabled.flag");
            if pref_file.exists() {
                is_disabled = true;
            }
        }

        if is_disabled {
            let _ = set_autostart_registered(false);
        } else {
            // Default to enabled: keep pointer fresh to current_exe()
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
        let _ = set_autostart_registered(true);
        assert!(is_autostart_registered());

        let _ = set_autostart_registered(false);
        assert!(!is_autostart_registered());
    }
}

