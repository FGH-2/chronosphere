//! SSH/scp helpers for remote pivot execution and deploy.

use crate::engagement::Pivot;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct SshConn {
    pub target: String,
    pub port: u16,
    pub identity: Option<PathBuf>,
    pub password: Option<String>,
    pub control_path: PathBuf,
}

impl SshConn {
    pub fn from_pivot(pivot: &Pivot, engagement_dir: &Path) -> Result<Self> {
        let target = pivot
            .ssh_spec()
            .ok_or_else(|| anyhow::anyhow!("pivot '{}' missing ssh_user/ssh_host", pivot.name))?;
        let ssh_dir = engagement_dir.join(".ssh");
        std::fs::create_dir_all(&ssh_dir).ok();
        let control_path = ssh_dir.join(format!("cm-{}", pivot.name.replace('/', "_")));
        Ok(Self {
            target,
            port: pivot.ssh_port.unwrap_or(22),
            identity: pivot
                .ssh_identity
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            password: pivot
                .ssh_password
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned(),
            control_path,
        })
    }

    pub fn uses_password(&self) -> bool {
        self.password.is_some()
    }

    fn base_ssh_args(&self) -> Vec<String> {
        let cp = self.control_path.to_string_lossy().to_string();
        let mut v = vec![
            "-p".into(),
            self.port.to_string(),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            format!("ControlPath={}", cp),
            "-o".into(),
            "ControlPersist=10m".into(),
            "-o".into(),
            "ServerAliveInterval=30".into(),
        ];
        if let Some(id) = &self.identity {
            v.push("-i".into());
            v.push(id.to_string_lossy().into_owned());
        }
        v
    }

    fn wrap_prog(&self, prog: &str, args: &[String]) -> String {
        let mut cmd = String::new();
        if let Some(pw) = &self.password {
            cmd.push_str("sshpass -p ");
            cmd.push_str(&shell_escape(pw));
            cmd.push(' ');
        }
        cmd.push_str(prog);
        for a in args {
            cmd.push(' ');
            cmd.push_str(&shell_escape(a));
        }
        cmd
    }

    pub fn scp_to_remote(&self, local: &Path, remote_path: &str) -> Result<()> {
        let mut args = self.base_ssh_args();
        args.push(local.to_string_lossy().into_owned());
        args.push(format!("{}:{}", self.target, remote_path));
        let shell = self.wrap_prog("scp", &args);
        let status = Command::new("bash")
            .arg("-lc")
            .arg(&shell)
            .status()
            .with_context(|| format!("scp to {}", remote_path))?;
        if !status.success() {
            bail!("scp failed with status {:?}", status.code());
        }
        Ok(())
    }

    /// Build a shell command that scp's a script, runs it on the pivot, logs locally, writes status.
    pub fn remote_script_wrapper(
        &self,
        local_script: &Path,
        remote_script: &str,
        log_path: &str,
        status_path: &str,
        interactive: bool,
    ) -> String {
        let log = shell_escape(log_path);
        let status = shell_escape(status_path);
        let target = shell_escape(&self.target);

        let mut scp_args = self.base_ssh_args();
        scp_args.push(local_script.to_string_lossy().into_owned());
        scp_args.push(format!("{}:{}", self.target, remote_script));
        let scp_cmd = self.wrap_prog("scp", &scp_args);

        let remote_exec = shell_escape(&format!(
            "chmod +x {remote_script} && bash {remote_script}; ec=$?; rm -f {remote_script}; exit $ec"
        ));
        let mut ssh_cmd = if self.password.is_some() {
            format!(
                "sshpass -p {} ssh",
                shell_escape(self.password.as_ref().unwrap())
            )
        } else {
            "ssh".into()
        };
        if interactive {
            ssh_cmd.push_str(" -tt");
        }
        for a in self.base_ssh_args() {
            ssh_cmd.push(' ');
            ssh_cmd.push_str(&shell_escape(&a));
        }
        ssh_cmd.push(' ');
        ssh_cmd.push_str(&target);
        ssh_cmd.push(' ');
        ssh_cmd.push_str(&remote_exec);

        if interactive {
            format!(
                "{scp_cmd} && {ssh_cmd}; echo $? > {status}",
                scp_cmd = scp_cmd,
                ssh_cmd = ssh_cmd,
                status = status,
            )
        } else {
            format!(
                r#"{scp_cmd} && {ssh_cmd} 2>&1 | tee -a {log}; ec=${{PIPESTATUS[0]}}; echo "$ec" > {status}; echo; echo '[chronosphere] remote command finished (exit '"$ec"'). Press Up to recall.'; exec ${{SHELL:-bash}}"#,
                scp_cmd = scp_cmd,
                ssh_cmd = ssh_cmd,
                log = log,
                status = status,
            )
        }
    }
}

pub fn ensure_sshpass() -> Result<()> {
    if which::which("sshpass").is_err() {
        bail!(
            "sshpass not installed (apt install sshpass / brew install sshpass).\n\
             Set ssh_identity on the pivot for key-based auth, or install sshpass for password auth."
        );
    }
    Ok(())
}

pub fn write_remote_script(local_path: &Path, user_command: &str) -> Result<()> {
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let body = format!(
        "#!/usr/bin/env bash\nset -euo pipefail\n{user_command}\n",
        user_command = user_command
    );
    std::fs::write(local_path, body)
        .with_context(|| format!("write remote script {}", local_path.display()))?;
    Ok(())
}

pub fn remote_script_path(job_id: &str) -> String {
    format!("/tmp/chrono-{}.sh", job_id)
}

pub fn shell_escape(s: &str) -> String {
    shell_words::quote(s).to_string()
}

/// Simple ssh/scp runner for deploy (pubkey, identity file, or sshpass password).
#[derive(Debug, Clone)]
pub struct SshDeploySession {
    pub port: u16,
    pub identity: Option<PathBuf>,
    pub password: Option<String>,
}

impl SshDeploySession {
    pub fn run_scp(&self, local: &Path, remote: &str) -> Result<()> {
        let mut cmd = self.base_cmd("scp");
        cmd.arg(local).arg(remote);
        let status = cmd.status().with_context(|| "scp")?;
        if !status.success() {
            bail!("scp failed with status {:?}", status.code());
        }
        Ok(())
    }

    pub fn run_ssh(&self, host: &str, remote_cmd: &str) -> Result<()> {
        let mut cmd = self.base_cmd("ssh");
        cmd.arg(host).arg(remote_cmd);
        let status = cmd.status().with_context(|| "ssh")?;
        if !status.success() {
            bail!("ssh '{}' failed with status {:?}", remote_cmd, status.code());
        }
        Ok(())
    }

    fn base_cmd(&self, prog: &str) -> Command {
        let mut cmd = if let Some(pw) = &self.password {
            let mut c = Command::new("sshpass");
            c.arg("-p").arg(pw).arg(prog);
            c
        } else {
            Command::new(prog)
        };
        cmd.arg("-P").arg(self.port.to_string());
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        if let Some(id) = &self.identity {
            cmd.arg("-i").arg(id);
        }
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_wrapper_contains_scp_and_ssh() {
        let conn = SshConn {
            target: "user@10.0.0.5".into(),
            port: 22,
            identity: None,
            password: None,
            control_path: PathBuf::from("/tmp/cm-test"),
        };
        let w = conn.remote_script_wrapper(
            Path::new("/tmp/a.sh"),
            "/tmp/chrono-id.sh",
            "/tmp/id.log",
            "/tmp/id.status",
            false,
        );
        assert!(w.contains("scp"));
        assert!(w.contains("ssh"));
        assert!(w.contains("tee"));
    }

    #[test]
    fn remote_wrapper_uses_sshpass_when_password_set() {
        let conn = SshConn {
            target: "user@10.0.0.5".into(),
            port: 22,
            identity: None,
            password: Some("s3cret".into()),
            control_path: PathBuf::from("/tmp/cm-test"),
        };
        let w = conn.remote_script_wrapper(
            Path::new("/tmp/a.sh"),
            "/tmp/chrono-id.sh",
            "/tmp/id.log",
            "/tmp/id.status",
            false,
        );
        assert!(w.contains("sshpass"));
        assert!(w.contains("s3cret"));
    }
}
