use directories::ProjectDirs;
use std::path::PathBuf;

pub const APP_QUALIFIER: &str = "dev";
pub const APP_ORG: &str = "chronosphere";
pub const APP_NAME: &str = "chronosphere";

pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(APP_QUALIFIER, APP_ORG, APP_NAME)
}

pub fn data_dir() -> PathBuf {
    project_dirs()
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chronosphere/data"))
}

pub fn config_dir() -> PathBuf {
    project_dirs()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chronosphere/config"))
}

pub fn log_file_path() -> PathBuf {
    data_dir().join("chronosphere.log")
}

pub fn engagements_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("engagements")
}

pub fn builtin_commands_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("commands")
}

pub fn last_engagement_marker() -> PathBuf {
    data_dir().join("last_engagement")
}

pub const TMUX_SESSION: &str = "chronosphere";
