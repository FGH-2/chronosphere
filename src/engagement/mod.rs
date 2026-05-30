pub mod creds;
pub mod history;
pub mod target;
pub mod variables;

pub use creds::{CredKind, CredentialProfile, ProfileStore};
pub use history::{HistoryStore, JobRecord, JobStatus};
pub use target::{Target, TargetStore};
pub use variables::VariableStore;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngagementMeta {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub notes: Option<String>,
}

/// A loaded engagement: holds its directory and the three mutable stores (targets, profiles, history).
pub struct Engagement {
    pub meta: EngagementMeta,
    pub dir: PathBuf,
    pub targets: TargetStore,
    pub profiles: ProfileStore,
    pub variables: VariableStore,
    pub history: HistoryStore,
}

impl Engagement {
    pub fn meta_path(dir: &Path) -> PathBuf {
        dir.join("engagement.toml")
    }
    pub fn targets_path(dir: &Path) -> PathBuf {
        dir.join("targets.json")
    }
    pub fn creds_path(dir: &Path) -> PathBuf {
        dir.join("creds.json")
    }
    pub fn variables_path(dir: &Path) -> PathBuf {
        dir.join("variables.json")
    }
    pub fn history_path(dir: &Path) -> PathBuf {
        dir.join("jobs.jsonl")
    }
    pub fn jobs_dir(dir: &Path) -> PathBuf {
        dir.join("jobs")
    }
    pub fn overrides_dir(dir: &Path) -> PathBuf {
        dir.join("commands")
    }

    pub fn create(root: &Path, name: &str) -> Result<Self> {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.starts_with('.')
        {
            anyhow::bail!("invalid engagement name '{}'", name);
        }
        let dir = root.join(name);
        if dir.exists() {
            anyhow::bail!("engagement '{}' already exists", name);
        }
        fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        fs::create_dir_all(Self::jobs_dir(&dir)).ok();
        fs::create_dir_all(Self::overrides_dir(&dir)).ok();

        let meta = EngagementMeta {
            name: name.to_string(),
            created_at: Utc::now(),
            notes: None,
        };
        fs::write(
            Self::meta_path(&dir),
            toml::to_string_pretty(&meta).context("serialize engagement meta")?,
        )?;
        let targets = TargetStore::new();
        targets.save(&Self::targets_path(&dir))?;
        let profiles = ProfileStore::new();
        profiles.save(&Self::creds_path(&dir))?;
        let variables = VariableStore::new();
        variables.save(&Self::variables_path(&dir))?;

        let history = HistoryStore::open(&Self::history_path(&dir))?;
        Ok(Self {
            meta,
            dir,
            targets,
            profiles,
            variables,
            history,
        })
    }

    pub fn load(dir: PathBuf) -> Result<Self> {
        let meta_path = Self::meta_path(&dir);
        let meta_str = fs::read_to_string(&meta_path)
            .with_context(|| format!("read {}", meta_path.display()))?;
        let meta: EngagementMeta =
            toml::from_str(&meta_str).context("parse engagement.toml")?;
        let targets = TargetStore::load(&Self::targets_path(&dir)).unwrap_or_else(|err| {
            tracing::warn!(?err, "could not load targets.json; starting fresh");
            TargetStore::new()
        });
        let profiles = ProfileStore::load(&Self::creds_path(&dir)).unwrap_or_else(|err| {
            tracing::warn!(?err, "could not load creds.json; starting fresh");
            ProfileStore::new()
        });
        let variables = VariableStore::load(&Self::variables_path(&dir)).unwrap_or_else(|err| {
            tracing::warn!(?err, "could not load variables.json; starting fresh");
            VariableStore::new()
        });
        let history = HistoryStore::open(&Self::history_path(&dir))?;
        fs::create_dir_all(Self::jobs_dir(&dir)).ok();
        fs::create_dir_all(Self::overrides_dir(&dir)).ok();
        Ok(Self {
            meta,
            dir,
            targets,
            profiles,
            variables,
            history,
        })
    }

    pub fn list(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(root) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() && Self::meta_path(&p).exists() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn active_target(&self) -> Option<&Target> {
        self.targets.active()
    }
    pub fn active_profile(&self) -> Option<&CredentialProfile> {
        self.profiles.active()
    }

    pub fn save_targets(&self) -> Result<()> {
        self.targets.save(&Self::targets_path(&self.dir))
    }
    pub fn save_profiles(&self) -> Result<()> {
        self.profiles.save(&Self::creds_path(&self.dir))
    }
    pub fn save_variables(&self) -> Result<()> {
        self.variables.save(&Self::variables_path(&self.dir))
    }
}
