use crate::engagement::{CredKind, CredentialProfile, Target};
use std::collections::BTreeMap;

/// Bag of values used by the placeholder expander. Anything the templates need can come from here.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    pub target: Option<Target>,
    pub profile: Option<CredentialProfile>,
    /// Global defaults (wordlist, userlist, etc.) keyed by placeholder name.
    pub globals: BTreeMap<String, String>,
}

impl RenderContext {
    pub fn resolve(&self, name: &str) -> Option<String> {
        match name {
            "target" => self
                .target
                .as_ref()
                .and_then(|t| t.ip.clone().or_else(|| t.hostname.clone())),
            "ip" => self.target.as_ref().and_then(|t| t.ip.clone()),
            "hostname" => self.target.as_ref().and_then(|t| t.hostname.clone()),
            /// Hostname-first address for HTTP URLs (e.g. sub1.target.htb); falls back to IP.
            "web_host" => self.web_host(),
            "web_base" => self.web_base(false),
            "web_base_https" => self.web_base(true),
            /// Root domain for vhost/subdomain fuzz (e.g. target.htb). Override with -v vhost_root=...
            "vhost_root" => self.vhost_root(),
            "dc" => self.target.as_ref().and_then(|t| t.dc_name.clone()),
            "dc_fqdn" => self.dc_fqdn(),
            "lhost" => self.target.as_ref().and_then(|t| t.lhost.clone()),
            "lport" => self
                .target
                .as_ref()
                .and_then(|t| t.lport.map(|p| p.to_string())),
            "domain" => self.profile.as_ref().and_then(|p| p.domain.clone()),
            "domain_upper" => self
                .profile
                .as_ref()
                .and_then(|p| p.domain.clone())
                .map(|d| d.to_uppercase()),
            "domain_short" => self
                .profile
                .as_ref()
                .and_then(|p| p.domain.clone())
                .and_then(|d| d.split('.').next().map(|s| s.to_string())),
            "user" => self.profile.as_ref().map(|p| p.username.clone()),
            "username" => self.profile.as_ref().map(|p| p.username.clone()),
            "password" => self.profile.as_ref().and_then(|p| match p.kind {
                CredKind::Plaintext => p.password.clone(),
                _ => None,
            }),
            "nt_hash" | "ntlm_hash" => self.profile.as_ref().and_then(|p| match p.kind {
                CredKind::Ntlm => p.nt_hash.clone(),
                _ => None,
            }),
            "ticket" | "kerberos_ticket" => self.profile.as_ref().and_then(|p| match p.kind {
                CredKind::Kerberos => p.ticket_path.clone(),
                _ => None,
            }),

            // derived
            "smb_user_spec" => Some(self.smb_user_spec()),
            "nxc_auth" => Some(self.nxc_auth()),
            "winrm_user" => self.winrm_user(),
            "domain_user" => self.domain_user(),
            "user_at_domain" => self.user_at_domain(),
            "cred_kind" => Some(
                self.profile
                    .as_ref()
                    .map(|p| p.kind.as_str())
                    .unwrap_or("none")
                    .to_string(),
            ),

            other => self.globals.get(other).cloned(),
        }
    }

    fn smb_user_spec(&self) -> String {
        match self.profile.as_ref() {
            None => "guest%".to_string(),
            Some(p) => match p.kind {
                CredKind::Plaintext => match (&p.domain, &p.password) {
                    (Some(d), Some(pw)) => format!("{}\\{}%{}", d, p.username, pw),
                    (None, Some(pw)) => format!("{}%{}", p.username, pw),
                    _ => format!("{}%", p.username),
                },
                CredKind::Ntlm => match &p.domain {
                    Some(d) => format!("{}\\{}%", d, p.username),
                    None => format!("{}%", p.username),
                },
                CredKind::Kerberos => "-k".into(),
                CredKind::None => "guest%".into(),
            },
        }
    }

    fn nxc_auth(&self) -> String {
        match self.profile.as_ref() {
            None => "-u 'guest' -p ''".to_string(),
            Some(p) => {
                let d_part = p
                    .domain
                    .as_deref()
                    .map(|d| format!(" -d {}", d))
                    .unwrap_or_default();
                match p.kind {
                    CredKind::Plaintext => format!(
                        "-u '{}' -p '{}'{}",
                        p.username,
                        p.password.as_deref().unwrap_or(""),
                        d_part
                    ),
                    CredKind::Ntlm => format!(
                        "-u '{}' -H '{}'{}",
                        p.username,
                        p.nt_hash.as_deref().unwrap_or(""),
                        d_part
                    ),
                    CredKind::Kerberos => "--use-kcache".to_string(),
                    CredKind::None => format!("-u '{}' -p ''", p.username),
                }
            }
        }
    }

    fn winrm_user(&self) -> Option<String> {
        let p = self.profile.as_ref()?;
        Some(match &p.domain {
            Some(d) => format!("{}\\{}", d, p.username),
            None => p.username.clone(),
        })
    }

    fn domain_user(&self) -> Option<String> {
        let p = self.profile.as_ref()?;
        match &p.domain {
            Some(d) => Some(format!("{}\\{}", d, p.username)),
            None => Some(p.username.clone()),
        }
    }

    fn dc_fqdn(&self) -> Option<String> {
        let t = self.target.as_ref()?;
        let dc = t.dc_name.clone()?;
        if dc.contains('.') {
            return Some(dc);
        }
        let dom = self.profile.as_ref().and_then(|p| p.domain.clone())?;
        Some(format!("{}.{}", dc, dom))
    }

    /// Prefer hostname for web URLs so vhosts like sub1.target.htb resolve correctly.
    fn web_host(&self) -> Option<String> {
        self.target
            .as_ref()
            .and_then(|t| t.hostname.clone().or_else(|| t.ip.clone()))
    }

    fn web_base(&self, https: bool) -> Option<String> {
        let host = self.web_host()?;
        Some(format!(
            "{}://{}",
            if https { "https" } else { "http" },
            host
        ))
    }

    /// Domain suffix for `Host: FUZZ.<root>` style scans (sub1.target.htb → target.htb).
    fn vhost_root(&self) -> Option<String> {
        if let Some(v) = self.globals.get("vhost_root") {
            return Some(v.clone());
        }
        let h = self.target.as_ref()?.hostname.as_ref()?;
        let parts: Vec<&str> = h.split('.').collect();
        if parts.len() <= 2 {
            return Some(h.clone());
        }
        Some(parts[1..].join("."))
    }

    fn user_at_domain(&self) -> Option<String> {
        let p = self.profile.as_ref()?;
        match &p.domain {
            Some(d) => Some(format!("{}@{}", p.username, d)),
            None => Some(p.username.clone()),
        }
    }

    /// Boolean atom lookup used by the `when` evaluator.
    pub fn lookup_bool(&self, dotted: &str) -> Option<bool> {
        match dotted {
            "target.has_dc" => Some(self.target.as_ref().and_then(|t| t.dc_name.clone()).is_some()),
            "target.has_ip" => Some(self.target.as_ref().and_then(|t| t.ip.clone()).is_some()),
            "target.has_hostname" => Some(
                self.target
                    .as_ref()
                    .and_then(|t| t.hostname.clone())
                    .is_some(),
            ),
            "target.has_lhost" => Some(self.target.as_ref().and_then(|t| t.lhost.clone()).is_some()),
            "creds.has_domain" => Some(self.profile.as_ref().and_then(|p| p.domain.clone()).is_some()),
            "creds.authenticated" => Some(matches!(
                self.profile.as_ref().map(|p| p.kind),
                Some(CredKind::Plaintext) | Some(CredKind::Ntlm) | Some(CredKind::Kerberos)
            )),
            _ => None,
        }
    }

    /// String atom lookup used by the `when` evaluator (e.g. `creds.kind == 'plaintext'`).
    pub fn lookup_string(&self, dotted: &str) -> Option<String> {
        match dotted {
            "creds.kind" => Some(
                self.profile
                    .as_ref()
                    .map(|p| p.kind.as_str())
                    .unwrap_or("none")
                    .to_string(),
            ),
            "creds.username" => self.profile.as_ref().map(|p| p.username.clone()),
            "creds.domain" => self.profile.as_ref().and_then(|p| p.domain.clone()),
            "target.name" => self.target.as_ref().map(|t| t.name.clone()),
            _ => None,
        }
    }
}
