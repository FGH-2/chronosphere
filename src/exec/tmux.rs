use anyhow::{Context, Result};
use std::process::Command;

pub fn has_tmux() -> bool {
    which::which("tmux").is_ok()
}

pub fn ensure_session(name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", name])
        .status()
        .context("tmux has-session")?;
    if status.success() {
        return Ok(());
    }
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", name, "-x", "220", "-y", "60"])
        .status()
        .context("tmux new-session")?;
    if !status.success() {
        anyhow::bail!("tmux new-session failed");
    }
    Ok(())
}

pub struct NewWindowOpts<'a> {
    pub session: Option<&'a str>,
    pub window_name: &'a str,
    pub detached: bool,
    pub command: &'a str,
}

/// Creates a tmux window running `command` in a fresh shell. Returns the resulting window id (e.g. `@5`).
pub fn new_window(opts: NewWindowOpts) -> Result<String> {
    let mut cmd = Command::new("tmux");
    cmd.arg("new-window");
    if opts.detached {
        cmd.arg("-d");
    }
    if let Some(s) = opts.session {
        cmd.args(["-t", s]);
    }
    cmd.args(["-n", opts.window_name]);
    cmd.args(["-P", "-F", "#{window_id}"]);
    cmd.arg(opts.command);
    let out = cmd.output().context("tmux new-window")?;
    if !out.status.success() {
        anyhow::bail!(
            "tmux new-window failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let wid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if wid.is_empty() {
        anyhow::bail!("tmux new-window returned empty window id");
    }
    Ok(wid)
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: String,
    pub name: String,
    pub pane_pid: Option<u32>,
    pub activity: bool,
}

pub fn list_windows(session: Option<&str>) -> Result<Vec<WindowInfo>> {
    let mut cmd = Command::new("tmux");
    cmd.arg("list-windows");
    if let Some(s) = session {
        cmd.args(["-t", s]);
    }
    cmd.args([
        "-F",
        "#{window_id}|#{window_name}|#{pane_pid}|#{window_activity_flag}",
    ]);
    let out = cmd.output().context("tmux list-windows")?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let mut v = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 2 {
            continue;
        }
        v.push(WindowInfo {
            id: parts[0].to_string(),
            name: parts[1].to_string(),
            pane_pid: parts.get(2).and_then(|p| p.parse().ok()),
            activity: parts.get(3).map(|s| *s == "1").unwrap_or(false),
        });
    }
    Ok(v)
}

pub fn select_window(session: Option<&str>, window_id: &str) -> Result<()> {
    let mut cmd = Command::new("tmux");
    cmd.arg("select-window");
    if let Some(s) = session {
        cmd.args(["-t", &format!("{}:{}", s, window_id)]);
    } else {
        cmd.args(["-t", window_id]);
    }
    let status = cmd.status().context("tmux select-window")?;
    if !status.success() {
        anyhow::bail!("tmux select-window failed");
    }
    Ok(())
}

pub fn kill_window(session: Option<&str>, window_id: &str) -> Result<()> {
    let mut cmd = Command::new("tmux");
    cmd.arg("kill-window");
    if let Some(s) = session {
        cmd.args(["-t", &format!("{}:{}", s, window_id)]);
    } else {
        cmd.args(["-t", window_id]);
    }
    let status = cmd.status().context("tmux kill-window")?;
    if !status.success() {
        anyhow::bail!("tmux kill-window failed");
    }
    Ok(())
}
