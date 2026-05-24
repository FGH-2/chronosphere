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

pub fn user_commands_dir() -> PathBuf {
    data_dir().join("commands")
}

/// Where chronosphere finds its built-in command library at runtime.
///
/// Resolution order:
///   1. `$CHRONOSPHERE_COMMANDS_DIR` if set and exists.
///   2. `$CARGO_MANIFEST_DIR/commands` if it exists (dev workflow).
///   3. `$XDG_DATA_HOME/chronosphere/commands` — populated from the embedded
///      copy on first run / version bump.
///   4. System paths: `/usr/local/share/chronosphere/commands`,
///      `/usr/share/chronosphere/commands`.
pub fn builtin_commands_dir() -> PathBuf {
    if let Ok(p) = std::env::var("CHRONOSPHERE_COMMANDS_DIR") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return pb;
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("commands");
    if dev.exists() {
        return dev;
    }
    let user = user_commands_dir();
    if user.exists() {
        return user;
    }
    for sys in ["/usr/local/share/chronosphere/commands", "/usr/share/chronosphere/commands"] {
        let p = PathBuf::from(sys);
        if p.exists() {
            return p;
        }
    }
    user
}

pub fn last_engagement_marker() -> PathBuf {
    data_dir().join("last_engagement")
}

pub const TMUX_SESSION: &str = "chronosphere";
