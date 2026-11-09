use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub computer_name: String,
    pub status: String,
    pub hardware: String,
    pub engine_version: String,
    pub transfers_completed: u32,
    pub volume_moved: String,
    pub speed_rating: String,
    pub processor: String,
    pub memory: String,
    pub remote_task: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransferHistory {
    pub transfers_completed: u32,
    pub total_bytes_moved: u64,
}

static SPEED_CACHE: OnceLock<String> = OnceLock::new();

pub fn get_stats_file_path() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(local_app_data).join("CP");
        let _ = fs::create_dir_all(&dir);
        dir.join("stats.json")
    } else {
        PathBuf::from("cp-stats.json")
    }
}

pub fn load_real_stats() -> TransferHistory {
    let path = get_stats_file_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(history) = serde_json::from_str::<TransferHistory>(&content) {
            return history;
        }
    }
    TransferHistory::default()
}

pub fn record_completed_transfer(bytes: u64) -> TransferHistory {
    let mut history = load_real_stats();
    history.transfers_completed += 1;
    history.total_bytes_moved += bytes;

    let path = get_stats_file_path();
    if let Ok(content) = serde_json::to_string_pretty(&history) {
        let _ = fs::write(path, content);
    }
    history
}

pub fn reset_real_stats() -> TransferHistory {
    let path = get_stats_file_path();
    let _ = fs::remove_file(path);
    TransferHistory::default()
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn probe_system_info() -> SystemInfo {
    let computer_name = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "GAURAV".to_string());

    let status = if crate::interceptor::is_interceptor_active() {
        if crate::interceptor::is_interceptor_enabled() {
            "ON (intercepting)".to_string()
        } else {
            "PAUSED (bypassed)".to_string()
        }
    } else {
        "ON (idle)".to_string()
    };

    let hardware_desc = detect_storage_topology();
    let speed_desc = detect_speed_rating();
    let stats = load_real_stats();
    let processor = detect_processor_name();
    let memory = detect_ram_capacity();
    let remote_task = crate::remote_task::get_task_status().map(|t| format!("{} ({})", t.version, t.status));

    SystemInfo {
        computer_name,
        status,
        hardware: hardware_desc,
        engine_version: crate::ota::resolve_current_version(),
        transfers_completed: stats.transfers_completed,
        volume_moved: format_bytes(stats.total_bytes_moved),
        speed_rating: speed_desc,
        processor,
        memory,
        remote_task,
    }
}

/// Zero-process dynamic hardware storage topology probe (reads registry & drive geometry in <0.5ms)
pub fn detect_storage_topology() -> String {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(disk_enum) = hklm.open_subkey("SYSTEM\\CurrentControlSet\\Services\\disk\\Enum") {
            let count: u32 = disk_enum.get_value("Count").unwrap_or(0);
            let mut detected = Vec::new();

            for i in 0..count {
                let key_name = format!("{}", i);
                if let Ok(dev_path) = disk_enum.get_value::<String, _>(&key_name) {
                    let sub_path = format!("SYSTEM\\CurrentControlSet\\Enum\\{}", dev_path);
                    let friendly: String = if let Ok(dev_key) = hklm.open_subkey(&sub_path) {
                        dev_key.get_value("FriendlyName").unwrap_or_default()
                    } else {
                        String::new()
                    };

                    let path_upper = dev_path.to_uppercase();
                    let friendly_upper = friendly.to_uppercase();
                    let is_nvme = path_upper.contains("NVME") || friendly_upper.contains("NVME");
                    let is_ssd = is_nvme || path_upper.contains("SSD") || friendly_upper.contains("SSD");

                    let brand = if !friendly.is_empty() {
                        friendly.split_whitespace().next().unwrap_or("")
                    } else {
                        ""
                    };

                    let disk_type = if is_nvme {
                        if !brand.is_empty() && brand != "NVMe" && brand != "Disk" {
                            format!("{} NVMe SSD", brand)
                        } else {
                            "NVMe SSD".to_string()
                        }
                    } else if is_ssd {
                        if !brand.is_empty() && brand != "Disk" {
                            format!("{} SSD", brand)
                        } else {
                            "SATA SSD".to_string()
                        }
                    } else {
                        "SATA HDD".to_string()
                    };

                    if !detected.contains(&disk_type) {
                        detected.push(disk_type);
                    }
                }
            }

            if !detected.is_empty() {
                let capacity = probe_total_storage_capacity();
                if !capacity.is_empty() {
                    return format!("{} ({})", detected.join(" + "), capacity);
                }
                return detected.join(" + ");
            }
        }
    }
    "NVMe SSD".to_string()
}

/// Probes total primary volume size in GB without subprocesses
fn probe_total_storage_capacity() -> String {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        let c_drive: Vec<u16> = "C:\\\0".encode_utf16().collect();
        let mut free_bytes_avail: u64 = 0;
        let mut total_bytes: u64 = 0;
        let mut total_free: u64 = 0;
        if GetDiskFreeSpaceExW(
            c_drive.as_ptr(),
            &mut free_bytes_avail,
            &mut total_bytes,
            &mut total_free,
        ) != 0 {
            let d_drive: Vec<u16> = "D:\\\0".encode_utf16().collect();
            let mut d_avail: u64 = 0;
            let mut d_total: u64 = 0;
            let mut d_free: u64 = 0;
            if GetDiskFreeSpaceExW(d_drive.as_ptr(), &mut d_avail, &mut d_total, &mut d_free) != 0 {
                total_bytes += d_total;
            }

            let gb = (total_bytes as f64) / (1024.0 * 1024.0 * 1024.0);
            if gb >= 900.0 && gb <= 1100.0 {
                return "1 TB".to_string();
            } else if gb >= 1800.0 && gb <= 2200.0 {
                return "2 TB".to_string();
            } else if gb >= 450.0 && gb <= 550.0 {
                return "512 GB".to_string();
            } else if gb >= 220.0 && gb <= 270.0 {
                return "256 GB".to_string();
            } else if gb >= 110.0 && gb <= 140.0 {
                return "128 GB".to_string();
            } else {
                return format!("{:.0} GB", gb.round());
            }
        }
    }
    String::new()
}

/// Zero-process dynamic RAM capacity detection using Win32 GlobalMemoryStatusEx
pub fn detect_ram_capacity() -> String {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut mem: MEMORYSTATUSEX = std::mem::zeroed();
        mem.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut mem) != 0 {
            let gb = (mem.ullTotalPhys as f64) / (1024.0 * 1024.0 * 1024.0);
            return format!("{:.0} GB RAM", gb.round());
        }
    }
    "8 GB RAM".to_string()
}

/// Zero-process dynamic processor detection using hardware CPUID instruction
pub fn detect_processor_name() -> String {
    let raw = get_cpu_brand();
    if raw.is_empty() {
        return "Intel Core Processor".to_string();
    }
    let cleaned = raw
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace("CPU @ ", "@")
        .replace("Dual-Core", "")
        .replace("Quad-Core", "")
        .trim()
        .to_string();

    if let Some(pos) = cleaned.find('@') {
        cleaned[..pos].trim().to_string()
    } else {
        cleaned
    }
}

/// Zero-process CPUID instruction probe (runs in <1 microsecond, no child processes, zero terminal flashing)
#[cfg(target_arch = "x86_64")]
fn get_cpu_brand() -> String {
    let mut brand = Vec::with_capacity(48);
    for function in 0x80000002..=0x80000004 {
        let res = std::arch::x86_64::__cpuid(function);
        for reg in [res.eax, res.ebx, res.ecx, res.edx] {
            brand.extend_from_slice(&reg.to_le_bytes());
        }
    }
    let s = String::from_utf8_lossy(&brand);
    s.trim_matches(char::from(0)).trim().to_string()
}

#[cfg(not(target_arch = "x86_64"))]
fn get_cpu_brand() -> String {
    "Modern Multi-Core Processor".to_string()
}

/// Dynamically determine PCIe interface generation rating using native CPUID
fn detect_speed_rating() -> String {
    SPEED_CACHE.get_or_init(|| {
        let cpu = get_cpu_brand();
        // 11th Gen+ Intel or Zen 3+ AMD (Ryzen 5000+) supports PCIe 4.0; 9th/10th Gen Intel supports PCIe 3.0
        if cpu.contains("11th Gen") 
            || cpu.contains("12th Gen") 
            || cpu.contains("13th Gen") 
            || cpu.contains("14th Gen") 
            || cpu.contains("Ryzen 5") 
            || cpu.contains("Ryzen 7") 
            || cpu.contains("Ryzen 9") 
        {
            "PCIe 4.0 / 7.0 GB/s".to_string()
        } else {
            "PCIe 3.0 / 3.4 GB/s".to_string()
        }
    }).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(12 * 1024 * 1024 * 1024), "12.00 GB");
    }

    #[test]
    fn test_cpu_brand_zero_process() {
        let brand = get_cpu_brand();
        println!("Native CPUID Brand: {}", brand);
        assert!(!brand.is_empty());
    }

    #[test]
    fn test_system_info_fresh_defaults() {
        let info = probe_system_info();
        assert!(info.engine_version.starts_with("0.0."));
        assert!(info.status == "ON (intercepting)" || info.status == "ON (idle)");
        println!("Hardware: {}", info.hardware);
        println!("Speed Rating: {}", info.speed_rating);
        assert!(!info.hardware.is_empty());
        assert!(!info.speed_rating.is_empty());
    }
}
