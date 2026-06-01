use crate::cve::model::CveRecord;
use crate::cve::rate_limit::HttpClient;
use crate::cve::store::CveStore;

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeSet;

const CIRCL_BASE: &str = "https://cve.circl.lu/api/vulnerability";

pub async fn enrich_cve(client: &HttpClient, store: &mut CveStore, cve_id: &str) -> Result<bool> {
    let url = format!("{CIRCL_BASE}/{cve_id}");
    let resp = client.get("circl", &url).await?;
    if resp.status().as_u16() == 404 {
        return Ok(false);
    }
    let root: Value = resp.json().await.context("parse circl json")?;
    let Some(rec) = parse_circl(&root, cve_id) else {
        return Ok(false);
    };
    store.upsert(&rec)?;
    Ok(true)
}

fn parse_circl(root: &Value, cve_id: &str) -> Option<CveRecord> {
    let summary = root
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if summary.is_empty() {
        return None;
    }
    let mut sources = BTreeSet::new();
    sources.insert("circl".into());
    Some(CveRecord {
        id: cve_id.to_string(),
        description: summary,
        sources,
        ..Default::default()
    })
}
