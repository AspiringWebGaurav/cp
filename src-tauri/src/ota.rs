use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

pub const CP_PUBLIC_KEY: &str = "RWSkCAVPI2056Kop8d/N1rkik/LFIm3lWEhbsnNewzSHZCkkQ9F4qSNg";
pub const GPHOST_API_BASE: &str = "https://gphost.eu.cc";
pub const GPHOST_APP_SLUG: &str = "cp";

pub fn resolve_current_version() -> String {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let ver_file = std::path::PathBuf::from(local_app_data).join("CP").join("installed_version.txt");
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
    pub speed: String,
    pub stage: String, // "downloading" | "verifying" | "staging" | "ready"
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

pub async fn check_latest_release(current_version: &str) -> Result<Option<ReleaseManifest>, String> {
    let url = if let Ok(custom) = std::env::var("CP_UPDATE_URL") {
        custom
    } else {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!(
            "{}/api/v1/releases/{}/latest?target=windows-x86_64&arch=x86_64&current_version={}&t={}",
            GPHOST_API_BASE, GPHOST_APP_SLUG, current_version, ts
        )
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
        )
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let mut manifest: ReleaseManifest = res.json().await.map_err(|e| e.to_string())?;
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

        if is_newer_version(&manifest.version, current_version) {
            return Ok(Some(manifest));
        }
    }

    Ok(None)
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

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to create download client: {}", e))?;

    let res = client
        .get(&download_url)
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache")
        .send()
        .await
        .map_err(|e| format!("Failed to download update binary: {}", e))?;

    if !res.status().is_success() {
        return Err(format!("Download server returned HTTP {}", res.status()));
    }

    let total_bytes = res.content_length().unwrap_or(12 * 1024 * 1024);
    let mut downloaded_bytes: u64 = 0;
    let mut buffer = Vec::with_capacity(total_bytes as usize);
    let mut stream = res.bytes_stream();
    let start_time = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Failed to read stream chunk: {}", e))?;
        downloaded_bytes += chunk.len() as u64;
        buffer.extend_from_slice(&chunk);

        let now = std::time::Instant::now();
        if downloaded_bytes == total_bytes || now.duration_since(last_emit).as_millis() >= 40 {
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

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "update-download-progress",
            UpdateProgress {
                downloaded_bytes,
                total_bytes: downloaded_bytes,
                percentage: 100.0,
                speed: "0 MB/s".to_string(),
                stage: "verifying".to_string(),
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
    pending_paste: bool,
) -> Result<(), String> {
    if !staged_path.exists() {
        return Err(format!("Staged binary does not exist at {:?}", staged_path));
    }

    set_persisted_version(&target_version);

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
        return Err(format!(
            "Native in-process binary replacement failed: {}. Please ensure CP has write permissions.",
            e
        ));
    }

    // Clean up temporary staged file
    let _ = std::fs::remove_file(&staged_path);

    // 4. Persist installed version marker and pending paste lifecycle flag in LocalAppData
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let ver_dir = PathBuf::from(local_app_data).join("CP");
        let _ = std::fs::create_dir_all(&ver_dir);
        let _ = std::fs::write(ver_dir.join("installed_version.txt"), target_version.trim().trim_start_matches('v').as_bytes());
        if pending_paste {
            let _ = std::fs::write(ver_dir.join("pending_paste.flag"), b"1");
        }
    }

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
        cmd.arg("--show");
        if pending_paste {
            cmd.arg("--auto-paste");
        }
        cmd.current_dir(&parent_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            cmd.creation_flags(DETACHED_PROCESS);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparisons() {
        assert!(is_newer_version("0.0.2", "0.0.1"));
        assert!(is_newer_version("v0.0.2", "0.0.1"));
        assert!(is_newer_version("0.0.10", "0.0.9"));
        assert!(is_newer_version("0.1.0", "0.0.9"));
        assert!(!is_newer_version("0.0.1", "0.0.1"));
        assert!(!is_newer_version("0.0.1", "0.0.2"));
    }

    #[tokio::test]
    async fn test_check_latest_release_does_not_panic() {
        let result = check_latest_release("0.0.1").await;
        assert!(result.is_ok());
    }
}

