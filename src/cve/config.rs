use crate::config;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveConfig {
    #[serde(default)]
    pub nvd: NvdConfig,
    #[serde(default)]
    pub osv: OsvConfig,
    #[serde(default)]
    pub epss: EpssConfig,
    #[serde(default)]
    pub circl: CirclConfig,
    #[serde(default)]
    pub sync: SyncConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvdConfig {
    #[serde(default = "default_nvd_interval")]
    pub min_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OsvConfig {
    #[serde(default = "default_osv_interval")]
    pub min_interval_ms: u64,
    #[serde(default = "default_osv_max")]
    pub max_enrich_per_sync: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EpssConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CirclConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_circl_interval")]
    pub min_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    #[serde(default = "default_years")]
    pub default_years: Vec<u16>,
}

fn default_nvd_interval() -> u64 {
    6000
}
fn default_osv_interval() -> u64 {
    250
}
fn default_osv_max() -> u32 {
    500
}
fn default_circl_interval() -> u64 {
    1000
}
fn default_true() -> bool {
    true
}
fn default_years() -> Vec<u16> {
    vec![2024, 2025, 2026]
}

impl Default for CveConfig {
    fn default() -> Self {
        Self {
            nvd: NvdConfig {
                min_interval_ms: default_nvd_interval(),
            },
            osv: OsvConfig {
                min_interval_ms: default_osv_interval(),
                max_enrich_per_sync: default_osv_max(),
            },
            epss: EpssConfig { enabled: true },
            circl: CirclConfig {
                enabled: false,
                min_interval_ms: default_circl_interval(),
            },
            sync: SyncConfig {
                default_years: default_years(),
            },
        }
    }
}

impl CveConfig {
    pub fn load() -> Self {
        let path = config::cve_config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(s) => toml::from_str(&s).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            Self::default()
        }
    }

    pub fn nvd_interval_ms(&self) -> u64 {
        if std::env::var("NVD_API_KEY").is_ok() {
            2000
        } else {
            self.nvd.min_interval_ms
        }
    }

    pub fn user_agent() -> String {
        std::env::var("CHRONOSPHERE_USER_AGENT").unwrap_or_else(|_| {
            format!(
                "chronosphere/{} (https://github.com/chronosphere)",
                env!("CARGO_PKG_VERSION")
            )
        })
    }

    pub fn nvd_api_key() -> Option<String> {
        std::env::var("NVD_API_KEY").ok()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncState {
    #[serde(default)]
    pub feeds: std::collections::BTreeMap<String, FeedState>,
    pub last_sync: Option<String>,
    pub last_osv_enrich: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeedState {
    pub sha256: Option<String>,
    pub last_modified: Option<String>,
    pub imported_at: Option<String>,
}

impl SyncState {
    pub fn load() -> Self {
        let path = config::cve_sync_state_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config::cve_sync_state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn feed_changed(&self, name: &str, sha256: &str) -> bool {
        self.feeds
            .get(name)
            .and_then(|f| f.sha256.as_deref())
            .is_none_or(|old| old != sha256)
    }

    pub fn mark_feed(&mut self, name: &str, sha256: &str, last_modified: Option<&str>) {
        self.feeds.insert(
            name.to_string(),
            FeedState {
                sha256: Some(sha256.to_string()),
                last_modified: last_modified.map(String::from),
                imported_at: Some(chrono::Utc::now().to_rfc3339()),
            },
        );
    }
}

pub fn ensure_cve_dirs() -> anyhow::Result<()> {
    fs::create_dir_all(config::cve_dir())?;
    if let Some(parent) = config::config_dir().parent() {
        fs::create_dir_all(parent).ok();
    }
    Ok(())
}
