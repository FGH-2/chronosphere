mod app;
mod clipboard;
mod config;
mod engagement;
mod exec;
mod library;
mod render;
mod ui;
mod vim;

use anyhow::{Context, Result};
use std::fs::OpenOptions;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing().context("init tracing")?;
    tracing::info!("starting chronosphere");
    let res = app::App::new()
        .await
        .context("init app")?
        .run()
        .await;
    if let Err(err) = &res {
        tracing::error!(?err, "app exited with error");
    }
    res
}

fn init_tracing() -> Result<()> {
    let log_path = config::log_file_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("open log file {}", log_path.display()))?;
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_ansi(false)
        .with_writer(file)
        .init();
    Ok(())
}
