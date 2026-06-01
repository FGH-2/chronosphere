use crate::cve::rate_limit::HttpClient;
use crate::cve::store::CveStore;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::io::BufRead;

const EPSS_URL: &str = "https://epss.cyentia.com/epss_scores-current.csv.gz";

pub async fn sync_epss(client: &HttpClient, store: &mut CveStore, progress: bool) -> Result<u64> {
    let resp = client.get("epss", EPSS_URL).await?;
    let bytes = resp.bytes().await?;
    let decoder = GzDecoder::new(&bytes[..]);
    let reader = std::io::BufReader::new(decoder);
    let mut count = 0u64;
    for (i, line) in reader.lines().enumerate() {
        let line = line.context("read epss line")?;
        if i == 0 || line.starts_with('#') {
            continue;
        }
        if progress && i > 0 && i % 50_000 == 0 {
            eprintln!("cve sync: EPSS processing row {i}…");
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            continue;
        }
        let cve_id = parts[0].trim();
        if !cve_id.starts_with("CVE-") {
            continue;
        }
        let score: f64 = parts[1].parse().unwrap_or(0.0);
        let percentile: f64 = parts[2].parse().unwrap_or(0.0);
        if store.update_epss(cve_id, score, percentile).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}
