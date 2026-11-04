use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub url: Option<String>,
    pub size: Option<u64>,
    pub signature: Option<String>,
}

pub const GPHOST_API_BASE: &str = "https://gphost.eu.cc";
pub const GPHOST_APP_SLUG: &str = "cp";

pub async fn check_latest_release(current_version: &str) -> Result<Option<ReleaseManifest>, String> {
    let url = format!(
        "{}/api/v1/releases/{}/latest?target=windows-x86_64&arch=x86_64&current_version={}",
        GPHOST_API_BASE, GPHOST_APP_SLUG, current_version
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .get(&url)
        .header("User-Agent", "CP-Engine/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        let manifest: ReleaseManifest = res.json().await.map_err(|e| e.to_string())?;
        if manifest.version != current_version {
            return Ok(Some(manifest));
        }
    }

    Ok(None)
}

