use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CveRecord {
    pub id: String,
    pub published: Option<String>,
    pub modified: Option<String>,
    pub description: String,
    pub cvss_v31: Option<f64>,
    pub cvss_v40: Option<f64>,
    pub severity: Option<String>,
    pub vector_v31: Option<String>,
    pub in_kev: bool,
    pub kev_date_added: Option<String>,
    pub kev_due_date: Option<String>,
    pub epss_score: Option<f64>,
    pub epss_percentile: Option<f64>,
    pub sources: BTreeSet<String>,
    pub products: Vec<CveProduct>,
    pub cwes: Vec<String>,
    pub references: Vec<CveReference>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CveProduct {
    pub vendor: String,
    pub product: String,
    pub version_start: Option<String>,
    pub version_end: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CveReference {
    pub url: String,
    pub source: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CveFilter {
    pub query: Option<String>,
    pub product: Option<String>,
    pub vendor: Option<String>,
    pub cwe: Option<String>,
    pub severity: Option<String>,
    pub kev_only: bool,
    pub min_epss: Option<f64>,
    pub since_days: Option<u32>,
    pub tag: Option<String>,
    pub limit: usize,
}

impl CveFilter {
    pub fn new() -> Self {
        Self {
            limit: 50,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NvdMonthField {
    #[default]
    Published,
    Modified,
}

/// Calendar month to sync via the NVD API (smaller than full-year feeds).
#[derive(Debug, Clone)]
pub struct MonthSync {
    pub year: u16,
    pub month: u8,
    pub field: NvdMonthField,
}

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub full: bool,
    pub years: Vec<u16>,
    /// When set, fetch only CVEs for this month via NVD API date-range queries.
    pub month: Option<MonthSync>,
    pub providers: Vec<String>,
    pub enrich_osv: bool,
    pub enrich_epss: bool,
    /// Print stage progress to stderr (CLI sync).
    pub progress: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    pub added: u64,
    pub updated: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CveStatus {
    pub total: u64,
    pub kev_count: u64,
    pub db_path: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub last_nvd_feed: Option<String>,
}

pub fn severity_from_score(score: f64) -> &'static str {
    if score >= 9.0 {
        "CRITICAL"
    } else if score >= 7.0 {
        "HIGH"
    } else if score >= 4.0 {
        "MEDIUM"
    } else if score > 0.0 {
        "LOW"
    } else {
        "NONE"
    }
}

pub fn normalize_cve_id(id: &str) -> Option<String> {
    let upper = id.trim().to_uppercase();
    if upper.starts_with("CVE-") && upper.len() >= 13 {
        Some(upper)
    } else {
        None
    }
}
