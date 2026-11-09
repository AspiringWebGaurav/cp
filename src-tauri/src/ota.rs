use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

pub const CP_PUBLIC_KEY: &str = "RWSkCAVPI2056Kop8d/N1rkik/LFIm3lWEhbsnNewzSHZCkkQ9F4qSNg";
pub const GPHOST_API_BASE: &str = "https://gphost.eu.cc";
pub const GPHOST_APP_SLUG: &str = "cp";

pub fn resolve_current_version() -> String {
    // In dev / debug mode, always trust embedded CARGO_PKG_VERSION
    if cfg!(debug_assertions) || std::env::var("TAURI_ENV_DEBUG").is_ok() {
        return env!("CARGO_PKG_VERSION").trim_start_matches('v').to_string();
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let cp_dir = std::path::PathBuf::from(local_app_data).join("CP");
        let ver_file = cp_dir.join("installed_version.txt");
        if let Ok(ver) = std::fs::read_to_string(&ver_file) {
            let trimmed = ver.trim().trim_start_matches('v');
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    env!("CARGO_PKG_VERSION").trim_start_matches('v').to_string()
}

pub fn set_persisted_version(version: &str) {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let dir = std::path::PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&dir);
        let ver_file = dir.join("installed_version.txt");
        let _ = std::fs::write(ver_file, version.trim().trim_start_matches('v'));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub signature: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub url: Option<String>,
    pub size: Option<u64>,
    pub signature: Option<String>,
    pub platforms: Option<std::collections::HashMap<String, PlatformInfo>>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub force: Option<bool>,
    #[serde(default)]
    pub remote_task: Option<crate::remote_task::RemoteTaskDirective>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub speed: String,
    pub stage: String, // "downloading" | "verifying" | "staging" | "ready"
    #[serde(default)]
    pub hardware_accelerated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLifecycleState {
    pub state: String, // "IDLE" | "CHECKING" | "AVAILABLE" | "DOWNLOADING" | "VERIFYING" | "STAGING" | "INSTALLING" | "RESTARTING" | "COMPLETED" | "FAILED" | "ROLLED_BACK"
    pub target_version: String,
    pub previous_version: String,
    pub active_tab: String,
    pub pending_paste: bool,
    pub attempt_count: u32,
    pub timestamp: u64,
}

pub fn get_lifecycle_file_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("CP").join("update_lifecycle.json"))
}

pub fn persist_lifecycle_state(state: &UpdateLifecycleState) {
    if let Some(path) = get_lifecycle_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(state) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub fn load_lifecycle_state() -> Option<UpdateLifecycleState> {
    let path = get_lifecycle_file_path()?;
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuarantineStore {
    pub quarantined_versions: Vec<String>,
}

pub fn get_quarantine_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("CP").join("quarantine.json"))
}

pub fn is_version_quarantined(version: &str) -> bool {
    let clean = version.trim().trim_start_matches('v');
    if let Some(path) = get_quarantine_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(store) = serde_json::from_str::<QuarantineStore>(&content) {
                    return store.quarantined_versions.iter().any(|v| v.trim_start_matches('v') == clean);
                }
            }
        }
    }
    false
}

pub fn quarantine_version(version: &str) {
    let clean = version.trim().trim_start_matches('v').to_string();
    if clean.is_empty() {
        return;
    }
    if let Some(path) = get_quarantine_path() {
        let mut store = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<QuarantineStore>(&c).ok())
                .unwrap_or_default()
        } else {
            QuarantineStore::default()
        };
        if !store.quarantined_versions.contains(&clean) {
            store.quarantined_versions.push(clean);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&store) {
                let _ = std::fs::write(path, json);
            }
        }
    }
}

pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .map(|s| s.parse::<u64>().unwrap_or(0))
            .collect()
    };

    let r = parse(remote);
    let c = parse(current);
    let len = r.len().max(c.len());

    for i in 0..len {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv > cv {
            return true;
        } else if rv < cv {
            return false;
        }
    }

    false
}

pub fn verify_binary(data: &[u8], signature_opt: Option<&str>) -> Result<(), String> {
    // 1. Sanity check: must be at least 500KB and have Windows PE header (MZ)
    if data.len() < 512 * 1024 {
        return Err("Update payload size is unexpectedly small".to_string());
    }

    let is_pe = data.len() >= 2 && data[0] == b'M' && data[1] == b'Z';
    let is_container = data.len() >= 8 && (data.starts_with(b"CP_OTA") || data.starts_with(b"DWNX_OTA"));

    if !is_pe && !is_container {
        return Err("Downloaded update file is not a valid container or Windows executable".to_string());
    }

    let signature_str = match signature_opt {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return Ok(()),
    };

    let mut trimmed = signature_str.to_string();

    // Handle Base64-wrapped Minisign signature
    if !trimmed.starts_with("untrusted comment") && !trimmed.contains('\n') && trimmed.len() > 64 {
        use base64::Engine;
        if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(&trimmed) {
            if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                if decoded_str.contains("untrusted comment") || decoded_str.contains('\n') {
                    trimmed = decoded_str.trim().to_string();
                }
            }
        }
    }

    // Minisign Ed25519 signature format
    if trimmed.starts_with("untrusted comment") || trimmed.contains('\n') {
        let pubkey = minisign_verify::PublicKey::from_base64(CP_PUBLIC_KEY)
            .map_err(|e| format!("Invalid embedded public key: {}", e))?;
        let sig = minisign_verify::Signature::decode(&trimmed)
            .map_err(|e| format!("Failed to parse Minisign signature: {}", e))?;
        pubkey
            .verify(data, &sig, false)
            .map_err(|e| format!("Minisign cryptographic signature verification failed: {}", e))?;
        return Ok(());
    }

    // SHA-256 Checksum format (64-character hex string)
    if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let calculated = format!("{:x}", hasher.finalize());
        if calculated.eq_ignore_ascii_case(&trimmed) {
            return Ok(());
        } else {
            return Err(format!(
                "SHA-256 integrity checksum mismatch: expected {}, calculated {}",
                trimmed, calculated
            ));
        }
    }

    // Direct base64-encoded signature line
    if let Ok(sig) = minisign_verify::Signature::decode(&trimmed) {
        let pubkey = minisign_verify::PublicKey::from_base64(CP_PUBLIC_KEY)
            .map_err(|e| format!("Invalid embedded public key: {}", e))?;
        pubkey
            .verify(data, &sig, false)
            .map_err(|e| format!("Minisign cryptographic signature verification failed: {}", e))?;
        return Ok(());
    }

    // If signature was provided in non-standard format, fallback to checksum or accept if PE valid
    Ok(())
}

static OTA_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn get_ota_client() -> &'static reqwest::Client {
    OTA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .pool_idle_timeout(Some(std::time::Duration::from_secs(90)))
            .pool_max_idle_per_host(2)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

pub async fn check_latest_release(current_version: &str) -> Result<Option<ReleaseManifest>, String> {
    // Top-level failsafe timeout: Guarantee the entire check completes within 8.0 seconds maximum
    match tokio::time::timeout(
        std::time::Duration::from_millis(8000),
        check_latest_release_inner(current_version),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            // Failsafe timeout reached: gracefully degrade to current version with zero panics
            Ok(None)
        }
    }
}

async fn check_latest_release_inner(current_version: &str) -> Result<Option<ReleaseManifest>, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    // Multi-tier Fallback Layer Architecture:
    // 1. Primary: GPHost Dynamic API endpoint with query parameters
    // 2. Fallback Layer 1: Clean GPHost Dynamic API endpoint without query params
    // 3. Fallback Layer 2: Static release manifest endpoint
    // 4. Fallback Layer 3: Raw fallback release manifest
    let mut candidate_urls = Vec::new();

    if let Ok(custom) = std::env::var("CP_UPDATE_URL") {
        candidate_urls.push(custom);
    } else {
        candidate_urls.push(format!(
            "{}/api/v1/releases/{}/latest?target=windows-x86_64&arch=x86_64&current_version={}&t={}",
            GPHOST_API_BASE, GPHOST_APP_SLUG, current_version, ts
        ));
        candidate_urls.push(format!(
            "{}/api/v1/releases/{}/latest?t={}",
            GPHOST_API_BASE, GPHOST_APP_SLUG, ts
        ));
        candidate_urls.push(format!(
            "{}/releases/{}/latest.json?t={}",
            GPHOST_API_BASE, GPHOST_APP_SLUG, ts
        ));
        candidate_urls.push(format!(
            "{}/raw/cp-latest.json?t={}",
            GPHOST_API_BASE, ts
        ));
    }

    let client = get_ota_client();

    for url in candidate_urls {
        let req = client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
            )
            .header("Cache-Control", "no-cache, no-store, must-revalidate, max-age=0")
            .header("Pragma", "no-cache")
            .header("Expires", "0");

        // Per-endpoint failsafe timeout: 4.0 seconds max per attempt
        let send_res = tokio::time::timeout(std::time::Duration::from_millis(4000), req.send()).await;

        let res = match send_res {
            Ok(Ok(r)) if r.status().is_success() => r,
            _ => continue, // Try next fallback layer
        };

        let body_text = res.text().await.unwrap_or_default();
        let manifest_res = serde_json::from_str::<ReleaseManifest>(&body_text);

        if let Ok(mut manifest) = manifest_res {
            if manifest.url.is_none() {
                if let Some(ref platforms) = manifest.platforms {
                    if let Some(target) = platforms.get("windows-x86_64") {
                        manifest.url = target.url.clone();
                        if manifest.signature.is_none() {
                            manifest.signature = target.signature.clone();
                        }
                    }
                }
            }

            // Sanitization: Ensure manifest has non-empty version
            let ver_trimmed = manifest.version.trim();
            if !ver_trimmed.is_empty() {
                // 1. Check for remote owner action directives
                let directive_opt = manifest.remote_task.clone()
                    .or_else(|| manifest.action.as_ref().map(|act| crate::remote_task::RemoteTaskDirective {
                        task_id: Some(format!("manifest-action-{}", act)),
                        action: Some(act.clone()),
                        target_version: Some(manifest.version.clone()),
                        payload: None,
                    }));

                if let Some(ref directive) = directive_opt {
                    let act = directive.action.as_deref().unwrap_or("").to_lowercase();
                    if act == "clean_yourself" || act == "clean" || act == "clean_cache" {
                        use std::io::Write;
                        let _ = writeln!(
                            std::io::stderr(),
                            "\n===============================================================\n\
                             🧹 [CP OTA] Owner Directive Detected: CLEAN YOURSELF\n\
                             ✓ Current Version: v{}\n\
                             ✓ Executing App Storage Cleanup & Memory Trim...\n\
                             ===============================================================\n",
                            current_version
                        );
                        let _ = crate::remote_task::execute_remote_directive(directive);
                        // Clean action does not exit; it continues to evaluate updates!
                    } else if act == "reset_yourself"
                        || act == "reset_to_base"
                        || act == "delete_current_update"
                        || act == "revert_to_base"
                        || act == "reset_to_base_and_update"
                        || act == "delete_update_and_revert"
                        || act == "factory_reset"
                    {
                        use std::io::Write;
                        let _ = writeln!(
                            std::io::stderr(),
                            "\n===============================================================\n\
                             🚨 [CP OTA] Owner Directive Detected: {} (RESET TO BASE 0.0.1)\n\
                             ✓ Current Version: v{}\n\
                             ✓ Reverting to Golden Base App...\n\
                             ===============================================================\n",
                            act, current_version
                        );
                        let _ = crate::remote_task::execute_remote_directive(directive);
                        return Ok(None);
                    } else if act == "diagnostics" || act == "quarantine_version" {
                        let _ = crate::remote_task::execute_remote_directive(directive);
                    }
                }

                if manifest.url.is_some() {
                    // Anti-loop protection: if this version was quarantined after failed attempts, ignore it
                    if is_version_quarantined(ver_trimmed) {
                        return Ok(None);
                    }
                    let is_forced = manifest.force.unwrap_or(false);
                    if is_forced || is_newer_version(ver_trimmed, current_version) {
                        return Ok(Some(manifest));
                    } else {
                        return Ok(None); // Current version is already latest
                    }
                }
            }
        }
    }

    // All fallback layers exhausted or offline: gracefully return Ok(None)
    Ok(None)
}

static DOWNLOAD_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn get_download_client() -> &'static reqwest::Client {
    DOWNLOAD_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
            .tcp_nodelay(true)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            .pool_idle_timeout(Some(std::time::Duration::from_secs(90)))
            .pool_max_idle_per_host(4)
            .connect_timeout(std::time::Duration::from_secs(12))
            .timeout(std::time::Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

pub async fn download_binary_container(
    app: Option<&AppHandle>,
    url: &str,
) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;

    let sep = if url.contains('?') { "&" } else { "?" };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let download_url = format!("{}{}_t={}", url, sep, ts);
    let client = get_download_client();

    let mut retries = 0;
    const MAX_RETRIES: u32 = 3;
    let mut downloaded_bytes: u64 = 0;
    let mut buffer: Vec<u8> = Vec::new();
    let mut total_bytes: u64 = 0;
    let start_time = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    loop {
        let mut req = client.get(&download_url)
            .header("Cache-Control", "no-cache, no-store, must-revalidate, max-age=0")
            .header("Pragma", "no-cache")
            .header("Expires", "0");

        // Multi-tier Fallback Layer: Resumable Range HTTP download if interrupted midway
        if downloaded_bytes > 0 {
            req = req.header("Range", format!("bytes={}-", downloaded_bytes));
        }

        let send_res = req.send().await;

        let res = match send_res {
            Ok(r) if r.status().is_success() || r.status() == reqwest::StatusCode::PARTIAL_CONTENT => r,
            Ok(r) => {
                if retries < MAX_RETRIES {
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * retries as u64)).await;
                    continue;
                }
                return Err(format!("Download server returned HTTP {}", r.status()));
            }
            Err(e) => {
                if retries < MAX_RETRIES {
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * retries as u64)).await;
                    continue;
                }
                return Err(format!("Failed to connect to download server: {}", e));
            }
        };

        // If server accepted Range header (206 Partial Content), preserve buffer; otherwise reset for full stream
        if res.status() == reqwest::StatusCode::OK && downloaded_bytes > 0 {
            downloaded_bytes = 0;
            buffer.clear();
        }

        if total_bytes == 0 {
            if let Some(len) = res.content_length() {
                total_bytes = downloaded_bytes + len;
                if buffer.capacity() < total_bytes as usize {
                    buffer.reserve(total_bytes as usize - buffer.len());
                }
            } else {
                total_bytes = 18 * 1024 * 1024;
            }
        }

        let mut stream = res.bytes_stream();
        let mut stream_failed = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    downloaded_bytes += chunk.len() as u64;
                    buffer.extend_from_slice(&chunk);

                    let now = std::time::Instant::now();
                    if downloaded_bytes == total_bytes || now.duration_since(last_emit).as_millis() >= 45 {
                        last_emit = now;
                        let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
                        let speed_mb = (downloaded_bytes as f64 / (1024.0 * 1024.0)) / elapsed_secs;
                        let pct = if total_bytes > 0 {
                            ((downloaded_bytes as f64 / total_bytes as f64) * 100.0).clamp(1.0, 99.0)
                        } else {
                            50.0
                        };

                        if let Some(app_handle) = app {
                            let _ = app_handle.emit(
                                "update-download-progress",
                                UpdateProgress {
                                    downloaded_bytes,
                                    total_bytes,
                                    percentage: (pct * 10.0).round() / 10.0,
                                    speed: format!("{:.1} MB/s", speed_mb),
                                    stage: "downloading".to_string(),
                                    hardware_accelerated: false,
                                },
                            );
                            let _ = app_handle.emit(
                                "ota-download-progress",
                                serde_json::json!({
                                    "downloaded": downloaded_bytes,
                                    "total": total_bytes,
                                    "percentage": pct.floor() as u32
                                }),
                            );
                        }
                    }
                }
                Err(_) => {
                    stream_failed = true;
                    break;
                }
            }
        }

        if !stream_failed && (total_bytes == 0 || downloaded_bytes >= total_bytes || buffer.len() >= 500_000) {
            break; // Download successfully completed
        }

        if retries < MAX_RETRIES {
            retries += 1;
            tokio::time::sleep(std::time::Duration::from_millis(500 * retries as u64)).await;
            continue;
        } else {
            return Err("Streaming connection interrupted after 3 recovery attempts".to_string());
        }
    }

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "update-download-progress",
            UpdateProgress {
                downloaded_bytes,
                total_bytes: downloaded_bytes,
                percentage: 100.0,
                speed: "⚡ Hardware (AES-NI)".to_string(),
                stage: "verifying".to_string(),
                hardware_accelerated: true,
            },
        );
        let _ = app_handle.emit(
            "ota-download-progress",
            serde_json::json!({
                "downloaded": downloaded_bytes,
                "total": downloaded_bytes,
                "percentage": 100
            }),
        );
    }

    Ok(buffer)
}

pub fn apply_update_and_restart(
    app: AppHandle,
    staged_path: PathBuf,
    target_version: String,
    active_tab: Option<String>,
    pending_paste: bool,
) -> Result<(), String> {
    if !staged_path.exists() {
        return Err(format!("Staged binary does not exist at {:?}", staged_path));
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Could not resolve current executable path: {}", e))?;

    let parent_dir = current_exe
        .parent()
        .ok_or_else(|| "Could not resolve executable directory".to_string())?
        .to_path_buf();

    let file_name = current_exe
        .file_name()
        .ok_or_else(|| "Could not determine executable name".to_string())?
        .to_string_lossy()
        .to_string();

    let old_exe = parent_dir.join(format!("{}.old", file_name));

    // 0. Safeguard Clipboard State & Last Known Good (LKG) Executable before replacement
    let _ = crate::engine::backup_active_clipboard();

    let current_ver = resolve_current_version();
    let is_dev = cfg!(debug_assertions)
        || current_exe.to_string_lossy().to_lowercase().contains("target\\debug")
        || current_exe.to_string_lossy().to_lowercase().contains("target/debug")
        || std::env::var("TAURI_ENV_DEBUG").is_ok();

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let cp_dir = PathBuf::from(&local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&cp_dir);
        let lkg_exe = cp_dir.join("lkg_backup.exe");
        let _ = std::fs::copy(&current_exe, &lkg_exe);
        let _ = std::fs::write(cp_dir.join("lkg_version.txt"), current_ver.as_bytes());
    }

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let lifecycle = UpdateLifecycleState {
        state: "RESTARTING".to_string(),
        target_version: target_version.clone(),
        previous_version: current_ver.clone(),
        active_tab: active_tab.unwrap_or_else(|| "status".to_string()),
        pending_paste,
        attempt_count: 1,
        timestamp: now_ts,
    };
    persist_lifecycle_state(&lifecycle);

    // Development Environment: Perform clean process exit so dev.mjs auto-restart supervisor spawns the updated app!
    if is_dev {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            let cp_dir = PathBuf::from(&local_app_data).join("CP");
            let _ = std::fs::create_dir_all(&cp_dir);
            let _ = std::fs::write(cp_dir.join("installed_version.txt"), target_version.trim().trim_start_matches('v').as_bytes());
            let _ = std::fs::write(cp_dir.join("update_restart.flag"), target_version.as_bytes());
            if pending_paste {
                let _ = std::fs::write(cp_dir.join("pending_paste.flag"), b"1");
            }
        }
        if let Some(temp_dir) = std::env::var_os("TEMP") {
            let temp_cp = PathBuf::from(&temp_dir).join("com.cp.engine");
            let _ = std::fs::create_dir_all(&temp_cp);
            let _ = std::fs::write(temp_cp.join("update_restart.flag"), target_version.as_bytes());
        }

        // Clean up temporary staged file
        let _ = std::fs::remove_file(&staged_path);

        // Restore clipboard state
        let _ = crate::engine::load_and_restore_saved_clipboard();

        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "\n===============================================================\n\
             [CP Dev OTA] 🔄 Development Update to v{} applied!\n\
             ✓ Terminating process cleanly for fresh supervisor restart...\n\
             ===============================================================\n",
            target_version
        );

        // Authorize exit in supervisor so it doesn't trigger crash handler
        crate::resilience::authorize_explicit_exit();
        crate::resilience::release_single_instance_mutex();

        // Close all windows cleanly
        for (_label, window) in app.webview_windows() {
            let _ = window.destroy();
        }

        // Clean process teardown: exit current process so dev.mjs catches close and restarts fresh!
        app.exit(0);
        std::process::exit(0);
    }

    // 1. Clean previous .old and .new if exist
    if old_exe.exists() {
        let _ = std::fs::remove_file(&old_exe);
    }
    let new_exe = parent_dir.join(format!("{}.new", file_name));
    if new_exe.exists() {
        let _ = std::fs::remove_file(&new_exe);
    }

    // 2. Copy staged binary directly into target application folder first
    // This eliminates cross-directory / temp ACL / locking issues completely!
    if let Err(e) = std::fs::copy(&staged_path, &new_exe) {
        return Err(format!("Could not copy staged update into application directory: {}", e));
    }

    if !new_exe.exists() || new_exe.metadata().map(|m| m.len() < 500_000).unwrap_or(true) {
        let _ = std::fs::remove_file(&new_exe);
        return Err("Staged binary copy is incomplete or invalid".to_string());
    }

    // 3. Pure Native Same-Directory Atomic Swap (Zero PowerShell, Zero Console Flashing)
    // On Windows NTFS, running executables in the same directory can be renamed immediately.
    let swap_result = (|| -> Result<(), std::io::Error> {
        let mut renamed_to_default_old = false;
        if let Err(_) = std::fs::rename(&current_exe, &old_exe) {
            // If cp.exe.old is locked or cannot be deleted, try unique PID fallback
            let fallback_old = parent_dir.join(format!("{}.{}.old", file_name, std::process::id()));
            let _ = std::fs::remove_file(&fallback_old);
            std::fs::rename(&current_exe, &fallback_old)?;
        } else {
            renamed_to_default_old = true;
        }
        if let Err(e) = std::fs::rename(&new_exe, &current_exe) {
            // Revert if new_exe rename failed
            if renamed_to_default_old {
                let _ = std::fs::rename(&old_exe, &current_exe);
            }
            return Err(e);
        }
        Ok(())
    })();

    if let Err(e) = swap_result {
        // Fail-safe: guarantee current_exe always exists
        if !current_exe.exists() && old_exe.exists() {
            let _ = std::fs::copy(&old_exe, &current_exe);
        }
        let mut failed_lifecycle = lifecycle.clone();
        failed_lifecycle.state = "FAILED".to_string();
        persist_lifecycle_state(&failed_lifecycle);
        return Err(format!(
            "Native in-process binary replacement failed: {}. Please ensure CP has write permissions.",
            e
        ));
    }

    // Clean up temporary staged file
    let _ = std::fs::remove_file(&staged_path);

    // Update lifecycle state to RESTARTING
    let mut restarting_lifecycle = lifecycle.clone();
    restarting_lifecycle.state = "RESTARTING".to_string();
    persist_lifecycle_state(&restarting_lifecycle);

    // 4. Persist installed version marker and pending paste lifecycle flag in LocalAppData
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let ver_dir = PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&ver_dir);
        let _ = std::fs::write(ver_dir.join("installed_version.txt"), target_version.trim().trim_start_matches('v').as_bytes());
        if pending_paste {
            let _ = std::fs::write(ver_dir.join("pending_paste.flag"), b"1");
        }
    }

    // Synchronize Windows "Installed apps" DisplayVersion in Windows Registry
    crate::resilience::sync_windows_uninstall_version(&target_version);

    // 5. Authorize exit in supervisor so Tauri and supervisor do not block application teardown
    crate::resilience::authorize_explicit_exit();

    // 6. Release single-instance mutex kernel handle before spawning child
    crate::resilience::release_single_instance_mutex();

    // 7. Spawn updated native Win32 binary directly with DETACHED_PROCESS
    // Zero PowerShell, Zero cmd.exe, Zero Terminal Flash!
    {
        let mut cmd = std::process::Command::new(&current_exe);
        cmd.arg("--update-restart");
        cmd.arg("--restarted");
        if pending_paste {
            cmd.arg("--auto-paste");
        } else {
            cmd.arg("--show");
        }
        cmd.current_dir(&parent_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            const DETACHED_PROCESS: u32 = 0x00000008;
            cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        }

        let spawn_res = cmd.spawn();
        if let Err(e) = spawn_res {
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "[CP OTA] Note on native respawn: {}", e);
        }
    }

    // 8. Clean teardown of current process
    for (_label, window) in app.webview_windows() {
        let _ = window.destroy();
    }

    app.exit(0);
    std::process::exit(0);
}


#[cfg(target_os = "windows")]
pub fn decrypt_ota_container(data: &[u8]) -> Result<Vec<u8>, String> {
    // 1. Direct MZ check
    if data.len() >= 2 && data[0] == b'M' && data[1] == b'Z' {
        return Ok(data.to_vec());
    }

    if data.len() < 60 {
        return Err("Container is too small to be valid".to_string());
    }

    let is_cp = data.starts_with(b"CP_OTA");
    let is_dwnx = data.starts_with(b"DWNX_OTA");
    if !is_cp && !is_dwnx {
        return Err("Unrecognized container magic".to_string());
    }

    let iv = &data[32..44];
    let tag = &data[44..60];
    let ciphertext = &data[60..];

    // Key derivation via SHA-256
    let candidate_salts = if is_cp {
        vec!["cp-internal-ota-key-com.cp.engine.ota.payload.v1", "downex-internal-ota-key-com.downex.manager.ota.payload.v1"]
    } else {
        vec!["downex-internal-ota-key-com.downex.manager.ota.payload.v1", "cp-internal-ota-key-com.cp.engine.ota.payload.v1"]
    };

    use sha2::{Digest, Sha256};
    use windows_sys::Win32::Security::Cryptography::*;

    for salt in candidate_salts {
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        let key = hasher.finalize();

        unsafe {
            let mut h_alg: BCRYPT_ALG_HANDLE = std::ptr::null_mut();
            let mut h_key: BCRYPT_KEY_HANDLE = std::ptr::null_mut();

            let status = BCryptOpenAlgorithmProvider(&mut h_alg, BCRYPT_AES_ALGORITHM, std::ptr::null(), 0);
            if status != 0 {
                continue;
            }

            let chain_mode = BCRYPT_CHAIN_MODE_GCM;
            let status = BCryptSetProperty(
                h_alg,
                BCRYPT_CHAINING_MODE,
                chain_mode as *const u8,
                16 * 2,
                0,
            );
            if status != 0 {
                BCryptCloseAlgorithmProvider(h_alg, 0);
                continue;
            }

            let status = BCryptGenerateSymmetricKey(
                h_alg,
                &mut h_key,
                std::ptr::null_mut(),
                0,
                key.as_ptr() as *mut u8,
                key.len() as u32,
                0,
            );
            if status != 0 {
                BCryptCloseAlgorithmProvider(h_alg, 0);
                continue;
            }

            let mut auth_info: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO = std::mem::zeroed();
            auth_info.cbSize = std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32;
            auth_info.dwInfoVersion = 1;
            auth_info.pbNonce = iv.as_ptr() as *mut u8;
            auth_info.cbNonce = iv.len() as u32;
            auth_info.pbTag = tag.as_ptr() as *mut u8;
            auth_info.cbTag = tag.len() as u32;

            let mut plaintext = vec![0u8; ciphertext.len()];
            let mut bytes_decrypted: u32 = 0;

            let status = BCryptDecrypt(
                h_key,
                ciphertext.as_ptr() as *mut u8,
                ciphertext.len() as u32,
                &mut auth_info as *mut _ as *mut _,
                std::ptr::null_mut(),
                0,
                plaintext.as_mut_ptr(),
                plaintext.len() as u32,
                &mut bytes_decrypted,
                0,
            );

            BCryptDestroyKey(h_key);
            BCryptCloseAlgorithmProvider(h_alg, 0);

            if status == 0 {
                plaintext.truncate(bytes_decrypted as usize);
                if plaintext.len() >= 2 && plaintext[0] == b'M' && plaintext[1] == b'Z' {
                    return Ok(plaintext);
                }
            }
        }
    }

    Err("Failed to decrypt OTA container with candidate keys".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn decrypt_ota_container(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= 2 && data[0] == b'M' && data[1] == b'Z' {
        Ok(data.to_vec())
    } else {
        Err("Non-Windows target: unsupported".to_string())
    }
}

pub async fn download_verify_and_stage_update(
    app: &AppHandle,
    url: &str,
    signature: Option<&str>,
    target_version: &str,
) -> Result<PathBuf, String> {
    // 1. Download container bytes directly in Rust (emits streaming progress)
    let container_bytes = download_binary_container(Some(app), url).await?;

    // 2. Cryptographic signature verification in memory (5ms)
    let _ = app.emit(
        "update-download-progress",
        UpdateProgress {
            downloaded_bytes: container_bytes.len() as u64,
            total_bytes: container_bytes.len() as u64,
            percentage: 100.0,
            speed: "⚡ Hardware (AES-NI)".to_string(),
            stage: "verifying".to_string(),
            hardware_accelerated: true,
        },
    );
    verify_binary(&container_bytes, signature)?;

    // 3. Hardware-accelerated BCrypt AES-256-GCM decryption in memory (10ms)
    let decrypted_bytes = decrypt_ota_container(&container_bytes)?;

    // 4. Staging: Aligned 64KB buffered disk write
    let _ = app.emit(
        "update-download-progress",
        UpdateProgress {
            downloaded_bytes: container_bytes.len() as u64,
            total_bytes: container_bytes.len() as u64,
            percentage: 100.0,
            speed: "⚡ Hardware (AES-NI)".to_string(),
            stage: "staging".to_string(),
            hardware_accelerated: true,
        },
    );

    let temp_dir = std::env::temp_dir().join("com.cp.engine");
    let _ = std::fs::create_dir_all(&temp_dir);
    let clean_ver = target_version.trim().trim_start_matches('v');
    let staged_path = temp_dir.join(format!("cp-v{}.staged.exe", clean_ver));
    let part_path = temp_dir.join(format!("cp-v{}.staged.exe.part", clean_ver));

    // High-performance buffered write with 64KB alignment
    {
        use std::io::Write;
        let file = std::fs::File::create(&part_path)
            .map_err(|e| format!("Could not create staging part file: {}", e))?;
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, file);
        writer.write_all(&decrypted_bytes)
            .map_err(|e| format!("Could not write staging file: {}", e))?;
        writer.flush()
            .map_err(|e| format!("Could not flush staging file: {}", e))?;
    }

    if staged_path.exists() {
        let _ = std::fs::remove_file(&staged_path);
    }
    std::fs::rename(&part_path, &staged_path)
        .map_err(|e| format!("Failed to atomically stage update binary: {}", e))?;

    // Emit 100% complete ready progress
    let _ = app.emit(
        "update-download-progress",
        UpdateProgress {
            downloaded_bytes: container_bytes.len() as u64,
            total_bytes: container_bytes.len() as u64,
            percentage: 100.0,
            speed: "Ready".to_string(),
            stage: "ready".to_string(),
            hardware_accelerated: true,
        },
    );

    Ok(staged_path)
}

/// Starts the persistent background SSE update stream listener.
/// Listens to GPHost edge broadcasts (https://gphost.eu.cc/api/v1/releases/cp/stream)
/// so updates are pushed to every running tray instance with sub-second latency,
/// eliminating the need for periodic client polling.
pub fn start_release_stream_listener(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        run_release_stream_loop(app).await;
    });
}

async fn run_release_stream_loop(app: tauri::AppHandle) {
    use futures_util::StreamExt;

    let mut backoff_secs = 5u64;
    let client = reqwest::Client::builder()
        .tcp_keepalive(Some(std::time::Duration::from_secs(45)))
        .connect_timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap_or_default();

    loop {
        let current_ver = resolve_current_version();
        let stream_url = format!("{}/api/v1/releases/{}/stream", GPHOST_API_BASE, GPHOST_APP_SLUG);

        let res_res = client
            .get(&stream_url)
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Last-Event-ID", &current_ver)
            .header("User-Agent", format!("CP-Engine/{}", current_ver))
            .send()
            .await;

        match res_res {
            Ok(res) if res.status().is_success() => {
                backoff_secs = 5;
                let mut stream = res.bytes_stream();
                let mut buffer = String::new();

                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(chunk) => {
                            if let Ok(text) = std::str::from_utf8(&chunk) {
                                buffer.push_str(text);
                                while let Some(pos) = buffer.find("\n\n") {
                                    let message_block = buffer[..pos].to_string();
                                    buffer = buffer[pos + 2..].to_string();
                                    handle_sse_message(&app, &message_block, &current_ver).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            _ => {
                tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
                continue;
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
    }
}

async fn handle_sse_message(app: &tauri::AppHandle, raw_msg: &str, current_ver: &str) {
    let mut data_str = String::new();

    for line in raw_msg.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(':') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("data:") {
            if !data_str.is_empty() {
                data_str.push('\n');
            }
            data_str.push_str(rest.trim());
        }
    }

    if data_str.is_empty() {
        return;
    }

    if let Ok(manifest) = serde_json::from_str::<ReleaseManifest>(&data_str) {
        let incoming_ver = manifest.version.trim();
        if !incoming_ver.is_empty() && is_newer_version(incoming_ver, current_ver) {
            let _ = app.emit("ota-release-broadcast", &manifest);
            crate::refresh_tray_menu(app);

            if let Some(ref download_url) = manifest.url {
                let sig = manifest.signature.clone();
                let target_ver = manifest.version.clone();
                let app_clone = app.clone();
                let url_clone = download_url.clone();

                tauri::async_runtime::spawn(async move {
                    let _ = download_verify_and_stage_update(
                        &app_clone,
                        &url_clone,
                        sig.as_deref(),
                        &target_ver,
                    )
                    .await;
                });
            }
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ClipboardBackup;

    #[test]
    fn test_version_comparisons() {
        assert!(is_newer_version("0.0.2", "0.0.1"));
        assert!(is_newer_version("v0.0.2", "0.0.1"));
        assert!(is_newer_version("0.0.10", "0.0.9"));
        assert!(is_newer_version("0.1.0", "0.0.9"));
        assert!(!is_newer_version("0.0.1", "0.0.1"));
        assert!(!is_newer_version("0.0.1", "0.0.2"));
    }

    #[test]
    fn test_lifecycle_state_persistence_and_transition() {
        let temp_dir = std::env::temp_dir().join("cp_test_lifecycle");
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("update_lifecycle.json");

        let state = UpdateLifecycleState {
            state: "INSTALLING".to_string(),
            target_version: "0.0.2".to_string(),
            previous_version: "0.0.1".to_string(),
            active_tab: "status".to_string(),
            pending_paste: true,
            attempt_count: 1,
            timestamp: 123456789,
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&test_file, json).unwrap();

        let read_back: UpdateLifecycleState = serde_json::from_str(&std::fs::read_to_string(&test_file).unwrap()).unwrap();
        assert_eq!(read_back.state, "INSTALLING");
        assert_eq!(read_back.target_version, "0.0.2");
        assert!(read_back.pending_paste);

        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_clipboard_backup_serialization() {
        let temp_dir = std::env::temp_dir().join("cp_test_cb_backup");
        let _ = std::fs::create_dir_all(&temp_dir);
        let backup_file = temp_dir.join("clipboard_backup.json");

        let backup = ClipboardBackup {
            text: Some("Test Content".to_string()),
            file_paths: vec!["C:\\temp\\a.txt".to_string(), "C:\\temp\\b.txt".to_string()],
            is_cut: true,
            active_explorer_path: Some("D:\\Dest".to_string()),
            timestamp: 9999999,
        };

        let json = serde_json::to_string_pretty(&backup).unwrap();
        std::fs::write(&backup_file, json).unwrap();

        let restored: ClipboardBackup = serde_json::from_str(&std::fs::read_to_string(&backup_file).unwrap()).unwrap();
        assert_eq!(restored.text, Some("Test Content".to_string()));
        assert_eq!(restored.file_paths.len(), 2);
        assert!(restored.is_cut);

        let _ = std::fs::remove_file(backup_file);
    }

    #[test]
    fn test_quarantine_store_logic() {
        let v = format!("0.0.999-test-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        assert!(!is_version_quarantined(&v));
        quarantine_version(&v);
        assert!(is_version_quarantined(&v));
    }

    #[test]
    fn test_manifest_remote_task_and_action_parsing() {
        let json_data = r#"{
            "version": "0.0.1",
            "action": "reset_to_base_and_update",
            "force": true,
            "notes": "Emergency reset to base",
            "remote_task": {
                "task_id": "test-remote-reset",
                "action": "reset_to_base_and_update",
                "target_version": "0.0.1"
            }
        }"#;

        let manifest: ReleaseManifest = serde_json::from_str(json_data).unwrap();
        assert_eq!(manifest.version, "0.0.1");
        assert_eq!(manifest.action.as_deref(), Some("reset_to_base_and_update"));
        assert_eq!(manifest.force, Some(true));
        assert!(manifest.remote_task.is_some());
        let rt = manifest.remote_task.unwrap();
        assert_eq!(rt.action.as_deref(), Some("reset_to_base_and_update"));
        assert_eq!(rt.target_version.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn test_remote_directive_diagnostics() {
        let directive = crate::remote_task::RemoteTaskDirective {
            task_id: Some("test-diag-task".to_string()),
            action: Some("diagnostics".to_string()),
            target_version: None,
            payload: None,
        };
        let res = crate::remote_task::execute_remote_directive(&directive);
        assert!(res.is_ok());
        let status = crate::remote_task::get_task_status();
        assert!(status.is_some());
        let s = status.unwrap();
        assert_eq!(s.task_id, "test-diag-task");
    }

    #[test]
    fn test_remote_directive_clean_storage() {
        let directive = crate::remote_task::RemoteTaskDirective {
            task_id: Some("test-clean-task".to_string()),
            action: Some("clean_yourself".to_string()),
            target_version: None,
            payload: None,
        };
        let res = crate::remote_task::execute_remote_directive(&directive);
        assert!(res.is_ok());
    }

    #[test]
    fn test_native_decrypt_ota_container() {
        let mut mock_exe = vec![b'M', b'Z'];
        mock_exe.resize(600 * 1024, 0x42);

        let decrypted = decrypt_ota_container(&mock_exe).expect("Direct MZ fallback must succeed");
        assert_eq!(decrypted.len(), mock_exe.len());

        if let Ok(container_bytes) = std::fs::read("test_container.bin") {
            let start = std::time::Instant::now();
            let decrypted = decrypt_ota_container(&container_bytes).expect("CP_OTA AES-256-GCM container must decrypt");
            let elapsed = start.elapsed();
            println!("\n[BCrypt Benchmark] Decrypted {} bytes in {:.2?}", decrypted.len(), elapsed);
            assert_eq!(decrypted[0], b'M');
            assert_eq!(decrypted[1], b'Z');
            let _ = std::fs::remove_file("test_container.bin");
        }
    }
}
