use crate::cve::model::CveFilter;
use rusqlite::types::Value;

/// Build SQL for CVE search combining FTS and structured filters.
pub fn build_search_query(filter: &CveFilter) -> (String, Vec<Value>) {
    let mut params: Vec<Value> = Vec::new();
    let mut where_clauses: Vec<String> = Vec::new();

    if let Some(ref q) = filter.query {
        let fts_q = q
            .split_whitespace()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(" ");
        where_clauses.push(format!(
            "c.id IN (SELECT id FROM cves_fts WHERE cves_fts MATCH ?{})",
            params.len() + 1
        ));
        params.push(Value::Text(fts_q));
    }

    if let Some(ref product) = filter.product {
        where_clauses.push(format!(
            "c.id IN (SELECT cve_id FROM cve_products WHERE product LIKE ?{} OR vendor LIKE ?{})",
            params.len() + 1,
            params.len() + 2
        ));
        let pat = format!("%{product}%");
        params.push(Value::Text(pat.clone()));
        params.push(Value::Text(pat));
    }

    if let Some(ref vendor) = filter.vendor {
        where_clauses.push(format!(
            "c.id IN (SELECT cve_id FROM cve_products WHERE vendor LIKE ?{})",
            params.len() + 1
        ));
        params.push(Value::Text(format!("%{vendor}%")));
    }

    if let Some(ref cwe) = filter.cwe {
        let cwe_norm = if cwe.starts_with("CWE-") {
            cwe.clone()
        } else {
            format!("CWE-{cwe}")
        };
        where_clauses.push(format!(
            "c.id IN (SELECT cve_id FROM cve_cwes WHERE cwe = ?{})",
            params.len() + 1
        ));
        params.push(Value::Text(cwe_norm));
    }

    if let Some(ref sev) = filter.severity {
        where_clauses.push(format!("UPPER(c.severity) = ?{}", params.len() + 1));
        params.push(Value::Text(sev.to_uppercase()));
    }

    if filter.kev_only {
        where_clauses.push("c.in_kev = 1".into());
    }

    if let Some(min) = filter.min_epss {
        where_clauses.push(format!("c.epss_score >= ?{}", params.len() + 1));
        params.push(Value::Real(min));
    }

    if let Some(days) = filter.since_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        where_clauses.push(format!("c.published >= ?{}", params.len() + 1));
        params.push(Value::Text(cutoff.format("%Y-%m-%d").to_string()));
    }

    if let Some(ref tag) = filter.tag {
        where_clauses.push(format!(
            "(c.description LIKE ?{} OR c.fts_text LIKE ?{})",
            params.len() + 1,
            params.len() + 2
        ));
        let pat = format!("%{tag}%");
        params.push(Value::Text(pat.clone()));
        params.push(Value::Text(pat));
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let limit = filter.limit.max(1).min(500);
    let sql = format!(
        "SELECT c.id FROM cves c {where_sql} ORDER BY c.modified DESC NULLS LAST, c.id DESC LIMIT {limit}"
    );
    (sql, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_kev_filter() {
        let f = CveFilter {
            kev_only: true,
            limit: 20,
            ..Default::default()
        };
        let (sql, _) = build_search_query(&f);
        assert!(sql.contains("in_kev = 1"));
    }
}
