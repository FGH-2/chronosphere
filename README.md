<!--

      ▄████████ ▄█    █▄     ▄████████  ▄██████▄  ███▄▄▄▄    ▄██████▄     ▄████████ ▄███████▄    ▄█    █▄       ▄████████    ▄████████    ▄████████
     ███    ███ ███    ███   ███    ███ ███    ███ ███▀▀▀██▄ ███    ███   ███    ███ ███   ███   ███   ███   ███    ███   ███    ███   ███    ███
     ███    █▀  ███    ███   ███    ███ ███    ███ ███   ███ ███    ███   ███    █▀  ███   ███   ███   ███   ███    █▀    ███    ███   ███    █▀
     ███        ███    ███  ▄███▄▄▄▄██▀ ███    ███ ███   ███ ███    ███   ███████████ ███▄▄▄██▀  ███▄▄▄███   ▄███▄▄▄      ▄███▄▄▄▄██▀   ███
     ███        ███    ███ ▀▀███▀▀▀▀▀   ███    ███ ███   ███ ███    ███          ███ ▀▀▀▀▀▀██▄  ▀▀▀▀▀▀███   ▀▀███▀▀▀     ▀▀███▀▀▀▀▀   ▀███████████
     ███    █▄  ███    ███ ▀███████████ ███    ███ ███   ███ ███    ███   ▄█    ███ ▄█▄ ▄██████        ███   ███    █▄  ▀███████████          ███
     ███    ███ ███    ███   ███    ███ ███    ███ ███   ███ ███    ███   ███    ███ ███   ███   ▄█    ███   ███    ███   ███    ███    ▄█    ███
     ████████▀   ▀██████▀    ███    ███  ▀██████▀   ▀█   █▀   ▀██████▀    ████████▀  ████████▀   ▀█████▀    ██████████   ███    ███  ▄████████▀
                             ███    ███                                                                                  ███    ███

-->

# chronosphere

A vim-flavored TUI + CLI + MCP server for browsing, templating, and firing pentest commands from a per-engagement project directory. Built for HTB / CTF / Pro Lab / engagement work where you need to swap targets and credentials constantly and keep all your tool invocations in one place.

> 「クロノスフィアは巡る。悪用（エクスプロイト）もまた同じ。」
> "The Chronosphere revolves. So does the exploit."

## Why

You don't want to grep your shell history for that one `certipy-ad` ESC1 invocation three days into a Pro Lab. You don't want to manually rewrite `-u 'user@domain'` across 40 commands when the active target changes. And you definitely don't want to copy-paste a `bash -c` reverse shell payload by hand.

chronosphere keeps every command you reach for — Impacket, NetExec, Certipy, BloodyAD, Responder, nmap, hashcat, the whole list — in declarative TOML files. The TUI renders them with your active target and credentials substituted in, fires them in background tmux windows, and tracks every job in a flat history per engagement.

The same binary also exposes those commands to AI agents (Cursor, Claude Desktop, anything MCP-aware) over stdio so the agent can pick the right invocation, run it, and tail the output — without having to memorize your tool of choice.

## Features

- **Vim TUI** with `hjkl` / `gg` / `G` / counts / marks / `:` / `/` / `?`.
- **Per-engagement state**: targets (with active selection + `tun0` auto-detect), credential profiles (plaintext / NTLM / Kerberos), job history, command overrides.
- **Declarative command library** (≈30 categories, hundreds of commands) in TOML, with conditional `when = "..."` clauses and `${fn:...}` helpers for reverse-shell generation, base64 encoding, etc.
- **Background execution** via tmux windows (auto-bootstraps a session) with per-job log files.
- **Inline edit + save-as** to fork a built-in command into your engagement's command directory.
- **Fuzzy search** (`/` local, `g/` global) over command id, title, tags, template.
- **CLI surface** mirroring the TUI for scripting / pipelines / dotfiles.
- **MCP server** (`chronosphere mcp-serve`) over stdio — drop into `~/.cursor/mcp.json` and let the agent drive.
- **`deploy` subcommand** that scp's the binary to a remote (Pwnbox / lab box) with pubkey or password auth.

## Quickstart

```bash
# 1. build
git clone <this repo> && cd chronosphere
cargo build --release         # target/release/chronosphere

# 2. drop it in $PATH
sudo install -m 0755 target/release/chronosphere /usr/local/bin/chronosphere
chronosphere update-templates  # extracts the built-in command TOMLs to your data dir

# 3. create an engagement
chronosphere new pro-lab-zephyr
chronosphere -e pro-lab-zephyr targets add dc01 --ip 10.10.11.5 --hostname dc.htb.local --dc DC01
chronosphere -e pro-lab-zephyr creds add admin --username Administrator --domain htb.local --kind plaintext --password 'hunter2'

# 4. launch the TUI (or stick in your shell aliases)
chronosphere -e pro-lab-zephyr
```

Inside the TUI, press `?` for the full keymap. The basics:

| key | what |
| --- | --- |
| `h` / `l` | move focus between categories ↔ commands ↔ preview ↔ jobs |
| `j` / `k` | move cursor down / up |
| `gg` / `G` | top / bottom |
| `r` | run the current command in a new tmux window |
| `5r` | run it five times (e.g. spray) |
| `R` | run every visible command (or every multi-selected one) |
| `y` | yank resolved command to clipboard |
| `Y` | yank raw template (with `{target}` etc.) |
| `e` | edit inline; `<C-s>` fires the edited command, `<C-w>` saves as a new id |
| `<space>` | toggle multi-select |
| `mX` / `'X` | set / jump to mark X |
| `/` | fuzzy search in the current category |
| `g/` | fuzzy search across all categories |
| `:` | command palette (`:engagement new <n>`, `:target add ...`, `:creds`, `:tools`, `:reload`, `:q`) |
| `?` | this help |

## Installation

### Local build (macOS, Linux)

Needs Rust ≥ 1.83 (edition 2024).

```bash
cargo install --path .                      # installs into ~/.cargo/bin/chronosphere
# or
cargo build --release && cp target/release/chronosphere /usr/local/bin/
```

First run extracts the embedded built-in command library (`commands/*.toml`) into your XDG data directory (`~/.local/share/chronosphere/commands` on Linux, `~/Library/Application Support/chronosphere/commands` on macOS). Edit those freely — your changes persist; rebuilds don't overwrite them. Use `chronosphere update-templates --force` to overwrite from the embedded set when the binary ships new commands.

### Cross-compile for Pwnbox (x86_64-linux from macOS)

Pwnbox runs Linux x86_64 and has very little spare disk for `cargo install`. Build a static musl binary once on your laptop instead:

```bash
rustup target add x86_64-unknown-linux-musl
cargo install --locked cargo-zigbuild     # uses zig as the cross-linker (~30 MB)
cargo zigbuild --release --target x86_64-unknown-linux-musl
file target/x86_64-unknown-linux-musl/release/chronosphere   # ELF 64-bit, ~4 MB
```

### Deploy to a remote (scp + bootstrap)

```bash
# Pubkey (uses your agent / ssh-config):
chronosphere deploy root@pwnbox.htb \
  --binary target/x86_64-unknown-linux-musl/release/chronosphere

# Password (Pwnbox gives you one per instance):
chronosphere deploy root@pwnbox.htb \
  --ask-password \
  --binary target/x86_64-unknown-linux-musl/release/chronosphere

# Need sudo to write into /usr/local/bin:
chronosphere deploy user@labbox --sudo \
  --binary target/x86_64-unknown-linux-musl/release/chronosphere

# Just want to see what it would do?
chronosphere deploy user@labbox --dry-run --binary <path>
```

`deploy` will refuse politely if you point it at a Mach-O binary by accident (it sniffs the magic bytes). For password auth it shells out to `sshpass`; if you don't have it installed you'll get a clear error. After upload it runs `chronosphere update-templates --force` on the remote and prints the next things to try.

## MCP server

Expose chronosphere to Cursor or any MCP client:

```bash
# Generate a snippet for the local binary:
chronosphere mcp-config

# Or for a remote Pwnbox / lab box (uses ssh ControlMaster — single persistent
# TCP session, near-zero SSH login spam in the agent log):
chronosphere mcp-config --ssh root@pwnbox.htb --port 22
```

Paste the inner `mcpServers` entry into `~/.cursor/mcp.json`. Restart Cursor and the agent will see ~19 tools:

| tool | what |
| --- | --- |
| `engagement_info` | which engagement / target / creds are loaded |
| `list_categories`, `list_commands(category?, tag?)` | browse the library |
| `search(query)` | substring + tag search |
| `show_command(id)` | full record incl. resolved template |
| `render_command(id, vars?, target?, creds?)` | render without firing |
| `run_command(id, vars?, dry_run?, timeout?)` | fire in background, returns `job_id` |
| `tail_job(job_id, lines?)` | tail the job's log |
| `grep_job(job_id, pattern)` | grep its log |
| `list_jobs(limit?)`, `kill_job(job_id)` | job history / SIGTERM |
| `targets_list`, `targets_add`, `targets_use` | target CRUD (auto-activates new entries) |
| `creds_list`, `creds_add`, `creds_use` | credential CRUD (auto-activates new entries) |
| `engagement_switch`, `engagement_new` | swap or create engagements without restart |
| `doctor(missing_only?)` | which referenced tools are present on this host |

The protocol is plain JSON-RPC 2.0 over stdio (`initialize`, `tools/list`, `tools/call`, `ping`), spec version `2024-11-05`. No SDK dependency — kept lean so the binary stays scp-able.

### Typical agent loop

`engagement_info` → `search` → `show_command` → `run_command` → `tail_job`. State-changing tools (`targets_add`, `creds_add`, `engagement_switch`) persist to disk immediately and become active. Background jobs return `job_id` so the agent can do other things while a scan runs.

## CLI reference

```
chronosphere [-e <engagement>] [-t <target>] [-c <creds>] [--root <dir>] <subcommand>

tui                       launch the TUI (default)
new <name> [--notes ...]  create an engagement
list <categories|commands|tools|engagements|jobs> [--category <id>]
show <command_id>         pretty-print one command resolved against current state
render <command_id>       print just the resolved command (shell-safe)
yank <command_id> [--raw] resolve + copy to clipboard
run <command_id> [-v K=V] [--dry-run]   execute (or print) the resolved command
search <query>            fuzzy search across the library

targets list|add|use|remove|show
creds   list|add|use|remove|show

doctor [--missing-only]   which referenced tools are installed
update-templates [--force]extract / refresh embedded commands into the user data dir
where                     print data dirs (commands, log, engagements)
path-install              print a shell snippet to put chronosphere on PATH

mcp-serve                 run as an MCP server over stdio
mcp-config [--ssh host]   print an mcp.json snippet (local or ssh-wrapped)
deploy <host> [opts...]   scp the binary to a remote and bootstrap it
```

Run `chronosphere <subcommand> --help` for the full option list of any subcommand.

## Command library

Commands live in TOML files, one category per file. The shape:

```toml
category = "impacket"
display_name = "Impacket"
order = 30

[[command]]
id = "imp.secretsdump.dcsync"
title = "secretsdump — DCSync"
tags = ["dcsync", "credentials"]
requires = ["impacket-secretsdump"]
when = "creds.kind in [plaintext, ntlm] && target.dc_name"
template = "impacket-secretsdump -just-dc-user '{user}' '{domain}/{user}:{password}@{dc_fqdn}'"
```

Placeholders are expanded from the active target + credential profile:

| placeholder | source |
| --- | --- |
| `{target}` | `target.ip` or `target.hostname` |
| `{ip}`, `{hostname}`, `{dc}`, `{dc_fqdn}` | target fields (`dc_fqdn` auto-built from `dc` + `domain`) |
| `{lhost}`, `{lport}` | target's local listener |
| `{user}`, `{domain}`, `{password}`, `{hash}` | credential profile fields |
| `{domain_upper}`, `{domain_short}` | derived (uppercase / first DNS label) |
| `${fn:bash_rev <lhost> <lport>}` | a helper function (see `src/render/helpers.rs`) |
| `{{literal}}` | escape — emits `{literal}` |

The `when` clause is evaluated against the same context and supports `==`, `!=`, `in`, `&&`, `||`. Unresolvable commands are filtered out of the visible list (until you set the right creds/target).

To add your own, drop a TOML file into `~/.local/share/chronosphere/commands/` (or `engagements/<name>/commands/` for per-engagement overrides). The file watcher reloads automatically.

## Layout

```
chronosphere/
├── src/
│   ├── app.rs              TUI app state & event loop
│   ├── cli.rs              clap subcommand surface
│   ├── deploy.rs           scp + ssh remote-install (pubkey / password)
│   ├── builtin.rs          include_dir! embed + extract of commands/*.toml
│   ├── mcp/                JSON-RPC 2.0 MCP server (mod.rs / protocol.rs / tools.rs)
│   ├── engagement/         engagement, targets, creds, job history
│   ├── library/            command library loader (recursive walkdir, TOML)
│   ├── render/             template engine + helper functions + when-clause evaluator
│   ├── exec/               tmux & external terminal spawning
│   ├── ui/                 ratatui panels, modals, theme, splash
│   └── vim.rs              vim-style key parser
├── commands/               built-in command library (embedded via include_dir!)
│   ├── recon.toml          nmap, masscan, dns, tlsx, ...
│   ├── smb.toml            nxc smb, smbclient, ...
│   ├── ad.toml             AD enum / kerberoast / spray
│   ├── impacket.toml       38 impacket commands
│   ├── adcs.toml           Certipy ESC1-ESC15 chains
│   ├── ad-postex.toml      bloodyAD, hashcat, john, Responder, mitm6
│   ├── ad-chains.toml      multi-step attack one-liners
│   ├── wsus.toml           rogue WSUS + ESC17
│   ├── kerberos.toml       getTGT / getST / s4u
│   ├── obfuscation.toml    PowerShell, donut, ScareCrow, ...
│   ├── cves.toml           nuclei / searchsploit / msfconsole
│   ├── web.toml            JWT, SSRF, OSINT, ffuf, hydra
│   ├── linux-privesc.toml  pspy, linpeas, GTFOBins picks
│   └── ...
└── README.md (you are here)
```

## Roadmap / out of scope

The following are sketched in the design notes but not implemented:

- **Local CVE index** with FTS5 + optional embeddings for description search.
- **PoC pipeline** that monitors NVD/KEV, pulls from `trickest/cve` etc., and surfaces them as runnable commands.
- **Obfuscation toolbelt** (PowerShell encoders, AMSI bypass templates, donut/ScareCrow wrappers).
- **Pro-Lab evasion preset** (defender recon, ETW disable hints, etc.).

If you need any of these urgently, open an issue and we'll talk.

## Logs / files

| what | where |
| --- | --- |
| engagements | `~/.local/share/chronosphere/engagements/` (Linux) or `~/Library/Application Support/chronosphere/engagements/` (macOS) |
| built-in commands (after first run) | `~/.local/share/chronosphere/commands/` |
| job logs (per engagement) | `<engagement>/jobs/<uuid>.log` |
| job history | `<engagement>/jobs.jsonl` |
| TUI log | `~/.local/state/chronosphere/log` |
| last-used engagement marker | `~/.local/state/chronosphere/last_engagement` |

## Credit

Designed and built by **CyberChronos** — [@CyberChronos00](https://x.com/CyberChronos00).

PRs welcome.
