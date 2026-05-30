use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
    Killed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub command_id: Option<String>,
    pub command_title: String,
    pub resolved: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub tmux_window: Option<String>,
    pub log_path: Option<PathBuf>,
    pub target: Option<String>,
    pub profile: Option<String>,
    #[serde(default)]
    pub ap: Option<String>,
}

pub struct HistoryStore {
    path: PathBuf,
    file: Mutex<File>,
    pub recent: Vec<JobRecord>,
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut recent = Vec::new();
        if path.exists() {
            let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
            for line in BufReader::new(file).lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<JobRecord>(&line) {
                    Ok(rec) => recent.push(rec),
                    Err(err) => tracing::warn!(?err, "skipping malformed jobs.jsonl line"),
                }
            }
        }
        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open append {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(f),
            recent,
        })
    }

    pub fn append(&mut self, rec: &JobRecord) -> Result<()> {
        let line = serde_json::to_string(rec).context("serialize JobRecord")?;
        let mut guard = self.file.lock().expect("history file mutex");
        writeln!(guard, "{}", line).with_context(|| format!("write {}", self.path.display()))?;
        guard.flush().ok();
        drop(guard);
        self.recent.push(rec.clone());
        Ok(())
    }

    pub fn update(&mut self, rec: &JobRecord) {
        if let Some(slot) = self.recent.iter_mut().find(|r| r.id == rec.id) {
            *slot = rec.clone();
        }
        self.rewrite_all();
    }

    fn rewrite_all(&mut self) {
        let tmp = self.path.with_extension("jsonl.tmp");
        let mut tmp_f = match File::create(&tmp) {
            Ok(f) => f,
            Err(err) => {
                tracing::warn!(?err, "rewrite history: create tmp failed");
                return;
            }
        };
        for r in &self.recent {
            if let Ok(line) = serde_json::to_string(r) {
                let _ = writeln!(tmp_f, "{}", line);
            }
        }
        drop(tmp_f);
        if let Err(err) = std::fs::rename(&tmp, &self.path) {
            tracing::warn!(?err, "rewrite history: rename failed");
        } else {
            // re-open append handle since we replaced the file
            if let Ok(f) = OpenOptions::new().append(true).open(&self.path) {
                *self.file.lock().expect("history file mutex") = f;
            }
        }
    }

    pub fn last_n(&self, n: usize) -> &[JobRecord] {
        let start = self.recent.len().saturating_sub(n);
        &self.recent[start..]
    }
}
