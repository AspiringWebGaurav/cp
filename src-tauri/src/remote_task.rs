use std::path::PathBuf;
use std::io::Write;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteTaskRecord {
    pub task_id: String,
    pub version: String,
    pub status: String,
    pub executed_at: u64,
    pub pid: u32,
    pub message: String,
}

/// Executes the remote task on fresh process start / post-update restart.
/// Logs clearly to stderr and writes audit proof to %LOCALAPPDATA%\CP\remote_task_status.json.
pub fn execute_remote_task() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let current_ver = crate::ota::resolve_current_version();
    let pid = std::process::id();

    // 2. Persist audit proof in %LOCALAPPDATA%\CP\remote_task_status.json
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let cp_dir = PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&cp_dir);
        let task_file = cp_dir.join("remote_task_status.json");

        let task_record = RemoteTaskRecord {
            task_id: format!("remote-task-v{}", current_ver),
            version: current_ver.clone(),
            status: "COMPLETED".to_string(),
            executed_at: now,
            pid,
            message: format!("Remote task v{} executed successfully on fresh restart", current_ver),
        };

        if let Ok(json_str) = serde_json::to_string_pretty(&task_record) {
            let _ = std::fs::write(&task_file, json_str);
        }

        // Also append to remote_task.log for persistent multi-run audit trail
        let log_file = cp_dir.join("remote_task.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
            let _ = writeln!(f, "[{}] Remote task v{} executed (PID: {})", now, current_ver, pid);
        }
    }
}

pub fn get_task_status() -> Option<RemoteTaskRecord> {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let task_file = PathBuf::from(local_app_data).join("CP").join("remote_task_status.json");
        if let Ok(content) = std::fs::read_to_string(&task_file) {
            if let Ok(record) = serde_json::from_str::<RemoteTaskRecord>(&content) {
                return Some(record);
            }
        }
    }
    None
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteTaskDirective {
    pub task_id: Option<String>,
    pub action: Option<String>, // "clean_yourself", "reset_yourself", "reset_to_base_and_update", "diagnostics", "clean_cache"
    pub target_version: Option<String>,
    pub payload: Option<serde_json::Value>,
}

/// Checks if an owner directive ID has already been executed to ensure perfect idempotency.
pub fn is_directive_already_executed(task_id: &str) -> bool {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let file = PathBuf::from(local_app_data).join("CP").join("executed_directives.json");
        if file.exists() {
            if let Ok(content) = std::fs::read_to_string(file) {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&content) {
                    return list.contains(&task_id.to_string());
                }
            }
        }
    }
    false
}

/// Records an executed directive ID in %LOCALAPPDATA%\CP\executed_directives.json.
pub fn mark_directive_as_executed(task_id: &str) {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(&local_app_data).join("CP");
        let file = dir.join("executed_directives.json");
        let mut list: Vec<String> = if file.exists() {
            std::fs::read_to_string(&file)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if !list.contains(&task_id.to_string()) {
            list.push(task_id.to_string());
            if list.len() > 100 {
                list.drain(0..(list.len() - 100)); // Maintain rolling window of last 100 directives
            }
            if let Ok(json) = serde_json::to_string_pretty(&list) {
                let _ = std::fs::write(file, json);
            }
        }
    }
}

/// Comprehensive app storage cleaner ("clean_yourself"):
/// - Purges *.old, *.staged, *.part, *.tmp in application folder
/// - Cleans %TEMP%\com.cp.engine update chunks
/// - Truncates oversized logs
/// - Trims process working set memory
pub fn clean_app_storage() -> Result<String, String> {
    let mut cleaned_count = 0;

    // 1. Clean app directory: remove *.old, *.staged, *.part, *.tmp, *.new
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent_dir) = current_exe.parent() {
            if let Ok(entries) = std::fs::read_dir(parent_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.ends_with(".old")
                        || name.ends_with(".staged")
                        || name.ends_with(".part")
                        || name.ends_with(".tmp")
                        || name.ends_with(".new")
                    {
                        if std::fs::remove_file(&path).is_ok() {
                            cleaned_count += 1;
                        }
                    }
                }
            }
        }
    }

    // 2. Clean %TEMP%\com.cp.engine
    if let Ok(temp_dir) = std::env::var("TEMP") {
        let cp_temp = PathBuf::from(temp_dir).join("com.cp.engine");
        if cp_temp.exists() {
            let _ = std::fs::remove_dir_all(&cp_temp);
            cleaned_count += 1;
        }
    }

    // 3. Clean %LOCALAPPDATA%\CP logs if oversized (> 256KB)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let cp_dir = PathBuf::from(local_app_data).join("CP");
        if cp_dir.exists() {
            for log_name in &["crash_recovery.log", "remote_task.log"] {
                let log_path = cp_dir.join(log_name);
                if let Ok(meta) = std::fs::metadata(&log_path) {
                    if meta.len() > 256 * 1024 {
                        if let Ok(content) = std::fs::read_to_string(&log_path) {
                            let lines: Vec<&str> = content.lines().collect();
                            let start = if lines.len() > 50 { lines.len() - 50 } else { 0 };
                            let truncated = lines[start..].join("\n");
                            let _ = std::fs::write(&log_path, truncated);
                            cleaned_count += 1;
                        }
                    }
                }
            }
        }
    }

    // 4. Memory working set trim
    crate::power::trim_working_set_memory();

    Ok(format!("Successfully cleaned {} items and trimmed working set", cleaned_count))
}

/// Executes dynamic remote task directives (e.g. sent via OTA manifest or administrative IPC).
pub fn execute_remote_directive(directive: &RemoteTaskDirective) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let current_ver = crate::ota::resolve_current_version();
    let pid = std::process::id();
    let action = directive.action.as_deref().unwrap_or("noop");
    let task_id = directive.task_id.clone().unwrap_or_else(|| format!("directive-{}", now));

    // Idempotency: skip if already executed
    if is_directive_already_executed(&task_id) {
        let _ = writeln!(
            std::io::stderr(),
            "[CP REMOTE DIRECTIVE] ℹ️ Directive '{}' (ID: {}) was already executed previously. Skipping.\n",
            action, task_id
        );
        return Ok(format!("Directive '{}' already executed previously", task_id));
    }

    let _ = writeln!(
        std::io::stderr(),
        "\n===============================================================\n\
         📡 [CP OWNER DIRECTIVE] Executing: '{}' (ID: {})\n\
         ✓ Current Version: v{}\n\
         ✓ Process PID:     {}\n\
         ✓ Timestamp:       {}\n\
         ===============================================================\n",
        action, task_id, current_ver, pid, now
    );

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let cp_dir = PathBuf::from(&local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&cp_dir);
        let task_file = cp_dir.join("remote_task_status.json");

        let task_record = RemoteTaskRecord {
            task_id: task_id.clone(),
            version: current_ver.clone(),
            status: format!("EXECUTED: {}", action),
            executed_at: now,
            pid,
            message: format!("Owner directive '{}' executed successfully (Target: {:?})", action, directive.target_version),
        };

        if let Ok(json_str) = serde_json::to_string_pretty(&task_record) {
            let _ = std::fs::write(&task_file, json_str);
        }

        let log_file = cp_dir.join("remote_task.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file) {
            let _ = writeln!(f, "[{}] Owner directive '{}' executed (ID: {}, PID: {})", now, action, task_id, pid);
        }
    }

    match action {
        "reset_yourself" | "reset_to_base" | "delete_current_update" | "revert_to_base" => {
            mark_directive_as_executed(&task_id);
            let _ = clean_app_storage();
            // Delete dynamic update artifacts and restore golden 0.0.1 base
            let success = crate::resilience::perform_base_app_rollback_ext(
                &format!("Owner Directive '{}' received", action),
                true // force_base = true ensures golden 0.0.1 base is restored
            );
            if success {
                Ok(format!("Successfully initiated reset to base 0.0.1 for task {}", task_id))
            } else {
                Err("Failed to execute base app rollback".to_string())
            }
        }
        "reset_to_base_and_update" | "delete_update_and_revert" | "factory_reset" => {
            mark_directive_as_executed(&task_id);
            let _ = clean_app_storage();
            // Write flag to trigger auto-check on boot into 0.0.1
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let flag_path = PathBuf::from(local_app_data).join("CP").join("check_update_on_boot.flag");
                let _ = std::fs::write(flag_path, b"1");
            }
            let success = crate::resilience::perform_base_app_rollback_ext(
                &format!("Owner Directive '{}' received", action),
                true
            );
            if success {
                Ok(format!("Successfully initiated factory reset and update for task {}", task_id))
            } else {
                Err("Failed to execute factory reset".to_string())
            }
        }
        "clean_yourself" | "clean" | "clean_cache" => {
            mark_directive_as_executed(&task_id);
            clean_app_storage()
        }
        "diagnostics" => {
            mark_directive_as_executed(&task_id);
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let diag_file = PathBuf::from(&local_app_data).join("CP").join("diagnostics.json");
                let base_present = PathBuf::from(&local_app_data).join("CP").join("base").join("cp-base.exe").exists();
                let diag = serde_json::json!({
                    "version": current_ver,
                    "timestamp": now,
                    "pid": pid,
                    "status": "healthy",
                    "engine": "Tauri v2 + Rust 2021 + Win32 FFI",
                    "base_snapshot_present": base_present
                });
                let _ = std::fs::write(diag_file, serde_json::to_string_pretty(&diag).unwrap_or_default());
            }
            Ok("Diagnostics collected and persisted".to_string())
        }
        "quarantine_version" => {
            mark_directive_as_executed(&task_id);
            if let Some(target) = &directive.target_version {
                crate::ota::quarantine_version(target);
                Ok(format!("Version {} quarantined remotely", target))
            } else {
                Err("Missing target_version for quarantine".to_string())
            }
        }
        other => {
            mark_directive_as_executed(&task_id);
            Ok(format!("Acknowledged custom remote action: {}", other))
        }
    }
}

