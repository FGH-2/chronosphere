//! Headless CLI surface. The TUI remains the default mode; subcommands are for
//! scripting and CI-friendly use (`chronosphere render`, `chronosphere doctor`,
//! `chronosphere new pl-zephyr`, etc.).

use crate::config;
use crate::engagement::{
    CredKind, CredentialProfile, Engagement, JobRecord, JobStatus, Target,
};
use crate::library::CommandLibrary;
use crate::render::{self, RenderContext};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "chronosphere", version, about = "Vim TUI + CLI for pentest engagement commands", long_about = None)]
pub struct Cli {
    /// Engagement name (defaults to last-used or single existing one).
    #[arg(short = 'e', long, global = true)]
    pub engagement: Option<String>,

    /// Override the engagement root directory.
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,

    /// Target name to apply for this invocation.
    #[arg(short = 't', long, global = true)]
    pub target: Option<String>,

    /// Credential profile name to apply for this invocation.
    #[arg(short = 'c', long, global = true)]
    pub creds: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Launch the TUI (default when no subcommand given).
    Tui,

    /// Create a new engagement directory.
    New {
        name: String,
        #[arg(long)]
        notes: Option<String>,
    },

    /// List engagements / categories / commands.
    List(ListArgs),

    /// Show a command (resolved with current target/creds).
    Show { id: String },

    /// Print the resolved command (shell-safe, single line).
    Render { id: String },

    /// Render and copy a command to the clipboard.
    Yank {
        id: String,
        /// Copy the raw template instead of the resolved command.
        #[arg(long)]
        raw: bool,
    },

    /// Execute a command (runs in current shell, prints output).
    Run {
        id: String,
        /// Replace `KEY=VALUE` placeholders for this run only.
        #[arg(short = 'v', long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        /// Don't actually execute; print the resolved command instead.
        #[arg(long)]
        dry_run: bool,
    },

    /// Search the command library.
    Search { query: String },

    /// Target CRUD.
    #[command(subcommand)]
    Targets(TargetCmd),

    /// Credential CRUD.
    #[command(subcommand)]
    Creds(CredsCmd),

    /// Check installed tools referenced by the library.
    Doctor {
        /// Only print missing tools (script-friendly).
        #[arg(long)]
        missing: bool,
    },

    /// Extract embedded built-in commands to the user data dir.
    UpdateTemplates {
        /// Overwrite local edits (default: keep local changes).
        #[arg(long)]
        force: bool,
    },

    /// Print where chronosphere stores things.
    Where,

    /// Print an `eval $(chronosphere path-install)` snippet for shell setup.
    PathInstall,

    /// Run as a Model Context Protocol server on stdio (for cursor-agent etc).
    McpServe,

    /// Print an mcp.json snippet to wire chronosphere into Cursor / Claude clients.
    McpConfig {
        /// Generate an SSH-wrapped server config for the named host (uses ControlMaster).
        #[arg(long)]
        ssh: Option<String>,
        /// SSH port for the remote (default 22).
        #[arg(long, default_value_t = 22)]
        port: u16,
        /// Path to the remote chronosphere binary.
        #[arg(long, default_value = "/usr/local/bin/chronosphere")]
        remote_path: String,
        /// Identity file for SSH (optional).
        #[arg(long)]
        identity: Option<PathBuf>,
    },

    /// Ship the chronosphere binary to a remote host via scp (Pwnbox, lab box, etc).
    Deploy(crate::deploy::DeployArgs),
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// What to list.
    #[arg(value_enum, default_value_t = ListKind::Categories)]
    pub kind: ListKind,
    /// Category id (when listing `commands`).
    pub category: Option<String>,
    /// Output JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum ListKind {
    Engagements,
    Categories,
    Commands,
    Tools,
    Jobs,
}

#[derive(Subcommand, Debug)]
pub enum TargetCmd {
    List,
    Add {
        name: String,
        #[arg(long)]
        ip: Option<String>,
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        dc: Option<String>,
        #[arg(long)]
        lhost: Option<String>,
        #[arg(long)]
        lport: Option<u16>,
        #[arg(long)]
        notes: Option<String>,
    },
    Use {
        name: String,
    },
    Remove {
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CredsCmd {
    List,
    Add {
        name: String,
        #[arg(long)]
        username: String,
        #[arg(long)]
        domain: Option<String>,
        /// One of: none|plaintext|ntlm|kerberos.
        #[arg(long, default_value = "plaintext")]
        kind: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        nt_hash: Option<String>,
        #[arg(long)]
        ticket: Option<PathBuf>,
        #[arg(long)]
        notes: Option<String>,
    },
    Use {
        name: String,
    },
    Remove {
        name: String,
    },
}

/// Returns true if dispatching consumed the run (CLI mode); false to fall through to TUI.
pub async fn dispatch(cli: Cli) -> Result<bool> {
    let cmd = match cli.command {
        Some(c) => c,
        None => return Ok(false),
    };
    if let Command::Tui = cmd {
        return Ok(false);
    }

    crate::builtin::ensure_user_dir().context("ensure user templates")?;

    let root = cli
        .root
        .clone()
        .unwrap_or_else(config::engagements_root);

    match cmd {
        Command::Tui => Ok(false),
        Command::New { name, notes } => {
            fs::create_dir_all(&root).ok();
            let mut e = Engagement::create(&root, &name)?;
            if let Some(n) = notes {
                e.meta.notes = Some(n);
                fs::write(
                    Engagement::meta_path(&e.dir),
                    toml::to_string_pretty(&e.meta)?,
                )?;
            }
            println!("created engagement: {}", e.dir.display());
            Ok(true)
        }
        Command::Where => {
            println!("data:        {}", config::data_dir().display());
            println!("config:      {}", config::config_dir().display());
            println!("log:         {}", config::log_file_path().display());
            println!("engagements: {}", root.display());
            println!("builtins:    {}", config::builtin_commands_dir().display());
            println!("user lib:    {}", config::user_commands_dir().display());
            Ok(true)
        }
        Command::PathInstall => {
            let bin = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "chronosphere".to_string());
            println!(
                r#"# add to ~/.zshrc or ~/.bashrc:
alias chronosphere='{bin}'
# or symlink:
#   sudo ln -sf '{bin}' /usr/local/bin/chronosphere
# tab completion for bash:
#   source <(chronosphere completions bash)"#
            );
            Ok(true)
        }
        Command::McpServe => {
            let opts = crate::mcp::ServerOpts {
                engagement: cli.engagement.clone(),
                root: root.clone(),
            };
            crate::mcp::serve(opts).await.context("mcp serve")?;
            Ok(true)
        }
        Command::McpConfig { ssh, port, remote_path, identity } => {
            if let Some(host) = ssh {
                println!(
                    "{}",
                    crate::deploy::mcp_config_ssh_snippet(&host, port, &remote_path, identity.as_deref())
                );
            } else {
                let bin = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.to_str().map(String::from))
                    .unwrap_or_else(|| "chronosphere".to_string());
                println!("{}", crate::deploy::mcp_config_local_snippet(&bin));
            }
            println!("\n# Paste the inner mcpServers entry into ~/.cursor/mcp.json");
            println!("# (or merge into an existing mcpServers object).");
            Ok(true)
        }
        Command::Deploy(deploy_args) => {
            crate::deploy::run(deploy_args).context("deploy")?;
            Ok(true)
        }
        Command::UpdateTemplates { force } => {
            let dir = config::user_commands_dir();
            let n = crate::builtin::extract_to(&dir, force)?;
            println!("extracted {} template files -> {}", n, dir.display());
            Ok(true)
        }
        Command::Doctor { missing } => {
            let sources = library_sources(root.as_path(), cli.engagement.as_deref())?;
            let lib = load_library(&sources)?;
            let tools = lib.all_tools_referenced();
            let mut found = 0usize;
            let mut not_found = Vec::new();
            for tool in &tools {
                if which::which(tool).is_ok() {
                    found += 1;
                } else {
                    not_found.push(tool.clone());
                }
            }
            not_found.sort();
            if missing {
                for t in &not_found {
                    println!("{}", t);
                }
            } else {
                println!("present: {} / {}", found, tools.len());
                println!("missing:");
                for t in &not_found {
                    println!("  - {}", t);
                }
            }
            Ok(true)
        }
        Command::List(args) => {
            let sources = library_sources(root.as_path(), cli.engagement.as_deref())?;
            let lib = load_library(&sources)?;
            match args.kind {
                ListKind::Engagements => {
                    for name in Engagement::list(&root) {
                        println!("{}", name);
                    }
                }
                ListKind::Categories => {
                    for cat in &lib.categories {
                        println!("{:<14}  {}", cat.id, cat.display_name);
                    }
                }
                ListKind::Commands => {
                    let filter = args.category.as_deref();
                    for cat in &lib.categories {
                        if filter.is_some_and(|f| f != cat.id) {
                            continue;
                        }
                        for cmd in &cat.commands {
                            println!("{:<18}  {}", cmd.id, cmd.title);
                        }
                    }
                }
                ListKind::Tools => {
                    let mut t: Vec<String> = lib.all_tools_referenced().into_iter().collect();
                    t.sort();
                    for tool in t {
                        println!("{}", tool);
                    }
                }
                ListKind::Jobs => {
                    let e = open_engagement(&root, cli.engagement.as_deref())?;
                    for job in e.history.recent.iter().rev() {
                        println!(
                            "{} {:?}  {}  ({})",
                            job.id,
                            job.status,
                            job.command_id.as_deref().unwrap_or("-"),
                            job.target.as_deref().unwrap_or("?")
                        );
                    }
                }
            }
            Ok(true)
        }
        Command::Search { query } => {
            let sources = library_sources(root.as_path(), cli.engagement.as_deref())?;
            let lib = load_library(&sources)?;
            let needle = query.to_lowercase();
            for cat in &lib.categories {
                for cmd in &cat.commands {
                    let hay = format!(
                        "{} {} {} {}",
                        cmd.id,
                        cmd.title,
                        cmd.tags.join(" "),
                        cmd.template,
                    )
                    .to_lowercase();
                    if hay.contains(&needle) {
                        println!("{:<18}  [{}]  {}", cmd.id, cat.id, cmd.title);
                    }
                }
            }
            Ok(true)
        }
        Command::Show { id } | Command::Render { id } => {
            let resolved = resolve(&root, cli.engagement.as_deref(), &cli.target, &cli.creds, &id, &[])?;
            println!("{}", resolved);
            Ok(true)
        }
        Command::Yank { id, raw } => {
            let text = if raw {
                raw_template(&root, cli.engagement.as_deref(), &id)?
            } else {
                resolve(&root, cli.engagement.as_deref(), &cli.target, &cli.creds, &id, &[])?
            };
            crate::clipboard::copy(&text)?;
            println!("yanked: {}", id);
            Ok(true)
        }
        Command::Run { id, vars, dry_run } => {
            let resolved = resolve(
                &root,
                cli.engagement.as_deref(),
                &cli.target,
                &cli.creds,
                &id,
                &vars,
            )?;
            if dry_run {
                println!("{}", resolved);
                return Ok(true);
            }
            run_with_history(&root, cli.engagement.as_deref(), &id, &resolved).await?;
            Ok(true)
        }
        Command::Targets(c) => {
            let mut e = open_engagement(&root, cli.engagement.as_deref())?;
            match c {
                TargetCmd::List => {
                    let active = e.targets.active().map(|t| t.name.clone());
                    for t in &e.targets.targets {
                        let star = if Some(&t.name) == active.as_ref() { "*" } else { " " };
                        println!(
                            "{} {:<14}  ip={}  host={}  dc={}",
                            star,
                            t.name,
                            t.ip.as_deref().unwrap_or("-"),
                            t.hostname.as_deref().unwrap_or("-"),
                            t.dc_name.as_deref().unwrap_or("-"),
                        );
                    }
                }
                TargetCmd::Add { name, ip, hostname, dc, lhost, lport, notes } => {
                    let activate = name.clone();
                    e.targets.upsert(Target {
                        name,
                        ip,
                        hostname,
                        dc_name: dc,
                        lhost,
                        lport,
                        notes,
                    });
                    e.targets.set_active(&activate);
                    e.save_targets()?;
                }
                TargetCmd::Use { name } => {
                    if !e.targets.set_active(&name) {
                        bail!("no target named {}", name);
                    }
                    e.save_targets()?;
                }
                TargetCmd::Remove { name } => {
                    e.targets.remove(&name);
                    e.save_targets()?;
                }
            }
            Ok(true)
        }
        Command::Creds(c) => {
            let mut e = open_engagement(&root, cli.engagement.as_deref())?;
            match c {
                CredsCmd::List => {
                    let active = e.profiles.active().map(|p| p.name.clone());
                    for p in &e.profiles.profiles {
                        let star = if Some(&p.name) == active.as_ref() { "*" } else { " " };
                        println!(
                            "{} {:<12} {}  ({}/{})  user={}",
                            star,
                            p.name,
                            p.kind.as_str(),
                            p.domain.as_deref().unwrap_or("-"),
                            p.username,
                            p.username,
                        );
                    }
                }
                CredsCmd::Add { name, username, domain, kind, password, nt_hash, ticket, notes } => {
                    let kind = match kind.to_lowercase().as_str() {
                        "none" => CredKind::None,
                        "plaintext" | "pw" | "password" => CredKind::Plaintext,
                        "ntlm" | "hash" | "nt" => CredKind::Ntlm,
                        "kerberos" | "krb" => CredKind::Kerberos,
                        other => bail!("unknown cred kind '{}' (none|plaintext|ntlm|kerberos)", other),
                    };
                    let activate = name.clone();
                    e.profiles.upsert(CredentialProfile {
                        name,
                        username,
                        domain,
                        kind,
                        password,
                        nt_hash,
                        ticket_path: ticket.map(|p| p.to_string_lossy().into_owned()),
                        notes,
                    });
                    e.profiles.set_active(&activate);
                    e.save_profiles()?;
                }
                CredsCmd::Use { name } => {
                    if !e.profiles.set_active(&name) {
                        bail!("no profile named {}", name);
                    }
                    e.save_profiles()?;
                }
                CredsCmd::Remove { name } => {
                    e.profiles.remove(&name);
                    e.save_profiles()?;
                }
            }
            Ok(true)
        }
    }
}

fn library_sources(root: &Path, engagement: Option<&str>) -> Result<Vec<PathBuf>> {
    let mut v = vec![config::builtin_commands_dir()];
    if let Some(name) = engagement {
        let dir = root.join(name);
        if dir.exists() {
            let overrides = Engagement::overrides_dir(&dir);
            if overrides.exists() {
                v.push(overrides);
            }
        }
    }
    Ok(v)
}

fn load_library(sources: &[PathBuf]) -> Result<CommandLibrary> {
    let paths: Vec<&Path> = sources.iter().map(|p| p.as_path()).collect();
    CommandLibrary::load(&paths)
}

fn open_engagement(root: &Path, name: Option<&str>) -> Result<Engagement> {
    let pick = match name {
        Some(n) => n.to_string(),
        None => {
            let candidates = Engagement::list(root);
            if candidates.len() == 1 {
                candidates.into_iter().next().unwrap()
            } else if candidates.is_empty() {
                bail!("no engagement found in {} (create one with `chronosphere new <name>`)", root.display())
            } else {
                bail!(
                    "ambiguous engagement (use -e to pick): {}",
                    candidates.join(", ")
                )
            }
        }
    };
    Engagement::load(root.join(&pick)).with_context(|| format!("load engagement {}", pick))
}

fn build_context(
    e: &Engagement,
    target_override: &Option<String>,
    cred_override: &Option<String>,
    extra_vars: &[String],
) -> RenderContext {
    let mut ctx = RenderContext::default();
    let active_t = target_override
        .as_deref()
        .and_then(|n| e.targets.targets.iter().find(|t| t.name == n))
        .or_else(|| e.targets.active());
    if let Some(t) = active_t {
        ctx.target = Some(t.clone());
    }
    let active_p = cred_override
        .as_deref()
        .and_then(|n| e.profiles.profiles.iter().find(|p| p.name == n))
        .or_else(|| e.profiles.active());
    if let Some(p) = active_p {
        ctx.profile = Some(p.clone());
    }
    for kv in extra_vars {
        if let Some((k, v)) = kv.split_once('=') {
            ctx.globals.insert(k.trim().to_string(), v.to_string());
        }
    }
    ctx
}

fn resolve(
    root: &Path,
    engagement: Option<&str>,
    target_override: &Option<String>,
    cred_override: &Option<String>,
    id: &str,
    extra_vars: &[String],
) -> Result<String> {
    let e = open_engagement(root, engagement)?;
    let sources = library_sources(root, Some(&e.meta.name))?;
    let lib = load_library(&sources)?;
    let cmd = lib
        .categories
        .iter()
        .flat_map(|c| c.commands.iter())
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("command id '{}' not found", id))?;
    let ctx = build_context(&e, target_override, cred_override, extra_vars);
    let tmpl = cmd.applicable_template(&|w| crate::render::condition::evaluate(w, &ctx));
    let result = render::render(tmpl, &ctx)?;
    Ok(result.resolved)
}

fn raw_template(root: &Path, engagement: Option<&str>, id: &str) -> Result<String> {
    let sources = library_sources(root, engagement)?;
    let lib = load_library(&sources)?;
    let cmd = lib
        .categories
        .iter()
        .flat_map(|c| c.commands.iter())
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("command id '{}' not found", id))?;
    Ok(cmd.template.clone())
}

async fn run_with_history(
    root: &Path,
    engagement: Option<&str>,
    id: &str,
    resolved: &str,
) -> Result<()> {
    use tokio::process::Command;
    let mut e = open_engagement(root, engagement)?;
    let target = e.targets.active().map(|t| t.name.clone());
    let profile = e.profiles.active().map(|p| p.name.clone());
    let started_at = Utc::now();
    let job_id = format!("{}", uuid::Uuid::new_v4());
    let log_path = Engagement::jobs_dir(&e.dir).join(format!("{job_id}.log"));
    fs::create_dir_all(Engagement::jobs_dir(&e.dir)).ok();
    eprintln!("[chrono] $ {}", resolved);
    let status = Command::new("bash")
        .arg("-lc")
        .arg(resolved)
        .status()
        .await
        .with_context(|| "spawn bash")?;
    let exit_code = status.code();
    let record = JobRecord {
        id: job_id,
        command_id: Some(id.to_string()),
        command_title: id.to_string(),
        resolved: resolved.to_string(),
        target,
        profile,
        started_at,
        finished_at: Some(Utc::now()),
        status: if status.success() {
            JobStatus::Completed
        } else {
            JobStatus::Failed
        },
        exit_code,
        log_path: Some(log_path),
        tmux_window: None,
    };
    e.history.append(&record)?;
    Ok(())
}
