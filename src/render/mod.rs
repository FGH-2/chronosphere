pub mod condition;
pub mod context;
pub mod helpers;

pub use context::RenderContext;

use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct RenderResult {
    pub resolved: String,
    pub unresolved: Vec<String>,
}

pub fn render(template: &str, ctx: &RenderContext) -> Result<RenderResult> {
    let after_placeholders = expand_placeholders(template, ctx);
    let resolved = helpers::expand_helpers(&after_placeholders.text)?;
    Ok(RenderResult {
        resolved,
        unresolved: after_placeholders.unresolved,
    })
}

struct PlaceholderPass {
    text: String,
    unresolved: Vec<String>,
}

/// Replaces `{name}` with the resolved value from `ctx`. Unknown names are left as `{name}` and
/// reported via the `unresolved` list so the UI can flag them.
///
/// Escape: `{{` and `}}` collapse to literal braces (so it's safe to keep things like awk programs in templates).
fn expand_placeholders(template: &str, ctx: &RenderContext) -> PlaceholderPass {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len());
    let mut unresolved = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '{' && i + 1 < bytes.len() && bytes[i + 1] as char == '{' {
            out.push('{');
            i += 2;
            continue;
        }
        if c == '}' && i + 1 < bytes.len() && bytes[i + 1] as char == '}' {
            out.push('}');
            i += 2;
            continue;
        }
        if c == '$' && i + 1 < bytes.len() && bytes[i + 1] as char == '{' {
            // leave ${...} sequences untouched here; helpers pass handles them.
            // copy to matching closing brace literally so `{` inside an inner ${} isn't grabbed.
            out.push('$');
            i += 1;
            continue;
        }
        if c == '{' {
            if let Some(end) = template[i + 1..].find('}') {
                let name = &template[i + 1..i + 1 + end];
                if name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && !name.is_empty()
                {
                    if let Some(val) = ctx.resolve(name) {
                        out.push_str(&val);
                    } else {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                        if !unresolved.iter().any(|x: &String| x == name) {
                            unresolved.push(name.to_string());
                        }
                    }
                    i += 1 + end + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    PlaceholderPass {
        text: out,
        unresolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engagement::{CredKind, CredentialProfile, Target};

    fn ctx() -> RenderContext {
        let mut c = RenderContext::default();
        c.target = Some(Target {
            name: "dc01".into(),
            ip: Some("10.10.11.10".into()),
            hostname: Some("DC01.CORP.LOCAL".into()),
            dc_name: Some("DC01".into()),
            lhost: Some("10.10.14.5".into()),
            lport: Some(4444),
            notes: None,
        });
        c.profile = Some(CredentialProfile {
            name: "jane".into(),
            username: "jane".into(),
            domain: Some("CORP".into()),
            kind: CredKind::Plaintext,
            password: Some("P@ss".into()),
            nt_hash: None,
            ticket_path: None,
            notes: None,
        });
        c
    }

    #[test]
    fn substitutes_known_placeholders() {
        let out = render("smbclient -L {target} -U '{user}'", &ctx()).unwrap();
        assert_eq!(out.resolved, "smbclient -L 10.10.11.10 -U 'jane'");
        assert!(out.unresolved.is_empty());
    }

    #[test]
    fn leaves_unknown_in_place_and_reports() {
        let out = render("nxc smb {target} {made_up}", &ctx()).unwrap();
        assert!(out.resolved.contains("{made_up}"));
        assert_eq!(out.unresolved, vec!["made_up".to_string()]);
    }

    #[test]
    fn double_braces_escape() {
        let out = render("awk '{{print $1}}' {target}", &ctx()).unwrap();
        assert!(out.resolved.starts_with("awk '{print $1}'"));
    }
}
