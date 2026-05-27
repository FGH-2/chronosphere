use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, Default)]
pub struct CopyReport {
    pub system_clipboard: bool,
    pub tmux_buffer: bool,
}

impl CopyReport {
    pub fn any(self) -> bool {
        self.system_clipboard || self.tmux_buffer
    }
}

fn copy_to_tmux_buffer(text: &str) -> Result<()> {
    // `set-buffer -w -` reads from stdin. This works even when no system clipboard is available
    // (e.g. SSH/headless) and lets users paste inside tmux with prefix `]`.
    let mut child = Command::new("tmux")
        .args(["set-buffer", "-w", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn tmux set-buffer")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .context("write tmux buffer")?;
    }
    let status = child.wait().context("wait tmux set-buffer")?;
    if !status.success() {
        anyhow::bail!("tmux set-buffer failed");
    }
    Ok(())
}

pub fn copy(text: &str) -> Result<()> {
    let r = copy_report(text)?;
    if r.any() {
        Ok(())
    } else {
        anyhow::bail!("no clipboard backend available")
    }
}

pub fn copy_report(text: &str) -> Result<CopyReport> {
    let mut r = CopyReport::default();

    // Try system clipboard first.
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if cb.set_text(text.to_string()).is_ok() {
            r.system_clipboard = true;
        }
    }

    // If inside tmux (or tmux exists), also copy to tmux buffer so paste works in the tmux job shell.
    if std::env::var("TMUX").is_ok() && Command::new("tmux").arg("-V").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false) {
        if copy_to_tmux_buffer(text).is_ok() {
            r.tmux_buffer = true;
        }
    }

    Ok(r)
}
