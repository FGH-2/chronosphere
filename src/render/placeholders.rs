use crate::library::CommandLibrary;
use std::collections::BTreeSet;

/// Placeholders expanded from target/credential profile (not stored in `variables.json`).
pub const BUILTIN_PLACEHOLDERS: &[&str] = &[
    "target",
    "ip",
    "hostname",
    "web_host",
    "web_base",
    "web_base_https",
    "vhost_root",
    "dc",
    "dc_fqdn",
    "lhost",
    "lport",
    "domain",
    "domain_upper",
    "domain_short",
    "user",
    "username",
    "password",
    "nt_hash",
    "ntlm_hash",
    "ticket",
    "kerberos_ticket",
    "smb_user_spec",
    "nxc_auth",
    "winrm_user",
    "domain_user",
    "user_at_domain",
    "cred_kind",
    "ssid",
    "bssid",
    "channel",
    "station",
    "wps_pin",
    "wpa_psk",
    "capture",
    "vendor",
    "pivot_host",
    "pivot_user",
    "pivot_name",
    "target_name",
    "pivot_ssh_port",
    "pivot_ssh_password",
    "pivot_ssh_key",
    "pivot_ssh_key_pub",
    "engagement_dir",
    "tun_iface",
    "ligolo_route",
    "ligolo_server_addr",
    "agent_path",
    "proxy_prefix",
    "execution_mode",
];

pub fn is_builtin_placeholder(name: &str) -> bool {
    BUILTIN_PLACEHOLDERS.contains(&name)
}

/// Extract `{name}` tokens from a template (`{{` / `}}` are literal braces).
pub fn extract_placeholders(template: &str) -> Vec<String> {
    let bytes = template.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '{' && i + 1 < bytes.len() && bytes[i + 1] as char == '{' {
            i += 2;
            continue;
        }
        if c == '}' && i + 1 < bytes.len() && bytes[i + 1] as char == '}' {
            i += 2;
            continue;
        }
        if c == '{' {
            if let Some(end) = template[i + 1..].find('}') {
                let name = &template[i + 1..i + 1 + end];
                if name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && !name.is_empty()
                    && !is_builtin_placeholder(name)
                    && !out.iter().any(|x: &String| x == name)
                {
                    out.push(name.to_string());
                }
                i += 1 + end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub fn collect_library_custom_placeholders(lib: &CommandLibrary) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for cat in &lib.categories {
        for cmd in &cat.commands {
            for n in extract_placeholders(&cmd.template) {
                names.insert(n);
            }
            for v in &cmd.variants {
                for n in extract_placeholders(&v.template) {
                    names.insert(n);
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_builtin_and_escaped() {
        let t = "hashcat -w {wordlist} {{literal}} {user} {target}";
        let names = extract_placeholders(t);
        assert_eq!(names, vec!["wordlist".to_string()]);
    }
}
