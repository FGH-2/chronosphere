use crate::config;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Name of the tmux paste buffer used for `y` yank (paste with prefix `]` or `paste-buffer -b chronosphere`).
pub const TMUX_BUFFER_NAME: &str = "chronosphere";

#[derive(Debug, Clone, Default)]
pub struct CopyReport {
    pub system_clipboard: bool,
    /// tmux internal paste buffer (prefix `]`), NOT the same as `-w` / system clipboard.
    pub tmux_buffer: bool,
    pub file_path: Option<PathBuf>,
}

impl CopyReport {
    pub fn any(&self) -> bool {
        self.system_clipboard || self.tmux_buffer || self.file_path.is_some()
    }
}

pub fn last_yank_path() -> PathBuf {
    config::data_dir().join("last_yank.txt")
}

pub fn format_yank_message(r: &CopyReport) -> String {
    let mut parts = Vec::new();
    if r.system_clipboard {
        parts.push("system clipboard");
    }
    if r.tmux_buffer {
        parts.push("tmux buffer (Ctrl-b ])");
    }
    if let Some(p) = &r.file_path {
        parts.push("saved to file");
        return if parts.len() == 1 {
            format!("yanked → {} (also: cat {})", parts[0], p.display())
        } else {
            format!(
                "yanked → {} (also: cat {})",
                parts.join(" + "),
                p.display()
            )
        }
    }
    if parts.is_empty() {
        "yank failed — no clipboard backend".to_string()
    } else {
        format!("yanked → {}", parts.join(" + "))
    }
}

fn tmux_available() -> bool {
    which::which("tmux").is_ok()
}

fn pipe_to_command(cmd: &mut Command, text: &str) -> Result<bool> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn clipboard command")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).context("write stdin")?;
    }
    Ok(child.wait().context("wait clipboard command")?.success())
}

/// tmux paste buffer (what `paste-buffer` / prefix `]` uses). Do NOT pass `-w` here.
fn copy_to_tmux_paste_buffer(text: &str) -> Result<()> {
    if !tmux_available() {
        anyhow::bail!("tmux not in PATH");
    }

    // Prefer load-buffer from a temp file (handles multiline / special chars reliably).
    let tmp = std::env::temp_dir().join(format!("chronosphere-yank-{}.txt", std::process::id()));
    std::fs::write(&tmp, text).context("write temp yank file")?;
    let loaded = Command::new("tmux")
        .args([
            "load-buffer",
            "-b",
            TMUX_BUFFER_NAME,
            tmp.to_str().context("temp path utf8")?,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&tmp);

    if loaded {
        return Ok(());
    }

    // Fallback: stdin → default buffer, then copy to named buffer.
    if !pipe_to_command(
        &mut Command::new("tmux").args(["set-buffer", "-"]),
        text,
    )? {
        anyhow::bail!("tmux set-buffer failed");
    }
    let _ = Command::new("tmux")
        .args(["copy-buffer", "-b", TMUX_BUFFER_NAME])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

#[cfg(target_os = "linux")]
fn copy_system_clipboard_linux(text: &str) -> bool {
    if which::which("xclip").is_ok() {
        for sel in ["clipboard", "primary"] {
            if pipe_to_command(
                &mut Command::new("xclip").args(["-selection", sel]),
                text,
            )
            .unwrap_or(false)
            {
                return true;
            }
        }
    }
    if which::which("xsel").is_ok() {
        for sel in ["--clipboard", "--primary"] {
            if pipe_to_command(&mut Command::new("xsel").arg(sel), text).unwrap_or(false) {
                return true;
            }
        }
    }
    if which::which("wl-copy").is_ok() {
        if pipe_to_command(&mut Command::new("wl-copy"), text).unwrap_or(false) {
            return true;
        }
    }
    // tmux can push to system clipboard when linked with X11/Wayland.
    if tmux_available() {
        if pipe_to_command(
            &mut Command::new("tmux").args(["set-buffer", "-w", "-"]),
            text,
        )
        .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn copy_system_clipboard(text: &str) -> bool {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if cb.set_text(text.to_string()).is_ok() {
            return true;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if copy_system_clipboard_linux(text) {
            return true;
        }
    }
    false
}

fn save_last_yank_file(text: &str) -> Result<PathBuf> {
    let path = last_yank_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, text).context("write last_yank.txt")?;
    Ok(path)
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

    if copy_system_clipboard(text) {
        r.system_clipboard = true;
    }

    // Always try tmux paste buffer when tmux exists (works even if Chronosphere is not inside tmux).
    if tmux_available() {
        if copy_to_tmux_paste_buffer(text).is_ok() {
            r.tmux_buffer = true;
        }
    }

    // Last-resort: always write file so user can `cat … | bash` or paste from editor.
    match save_last_yank_file(text) {
        Ok(p) => r.file_path = Some(p),
        Err(err) => tracing::warn!(?err, "could not write last_yank.txt"),
    }

    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_message_includes_tmux_hint() {
        let msg = format_yank_message(&CopyReport {
            tmux_buffer: true,
            ..Default::default()
        });
        assert!(msg.contains("tmux buffer"));
    }
}
