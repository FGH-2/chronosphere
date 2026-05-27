pub mod schema;

pub use schema::{CategoryFile, CommandEntry, CommandVariant};

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Aggregated, deduplicated command library (built-ins overridden by per-engagement files).
#[derive(Debug, Clone, Default)]
pub struct CommandLibrary {
    /// Ordered by `order` field, then alphabetically.
    pub categories: Vec<Category>,
}

#[derive(Debug, Clone, Default)]
pub struct Category {
    pub id: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub order: i32,
    pub commands: Vec<CommandEntry>,
    pub sources: Vec<PathBuf>,
}

impl CommandLibrary {
    pub fn load(sources: &[&Path]) -> Result<Self> {
        let mut cats: Vec<Category> = Vec::new();
        for src in sources {
            for entry in WalkDir::new(src).into_iter().flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let p = entry.path();
                if p.extension().map(|e| e == "toml").unwrap_or(false) {
                    match load_file(p) {
                        Ok(file) => merge_into(&mut cats, file, p.to_path_buf()),
                        Err(err) => {
                            tracing::error!(?err, file = %p.display(), "failed to load category");
                        }
                    }
                }
            }
        }
        cats.sort_by(|a, b| a.order.cmp(&b.order).then(a.display_name.cmp(&b.display_name)));
        Ok(Self { categories: cats })
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }

    pub fn all_tools_referenced(&self) -> HashSet<String> {
        let mut s = HashSet::new();
        for c in &self.categories {
            for cmd in &c.commands {
                for t in &cmd.requires {
                    s.insert(t.clone());
                }
            }
        }
        s
    }
}

fn load_file(path: &Path) -> Result<CategoryFile> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: CategoryFile =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn loads_real_command_library() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("commands");
        let lib = CommandLibrary::load(&[dir.as_path()]).expect("load library");
        assert!(
            lib.categories.len() >= 10,
            "expected lots of categories, got {}",
            lib.categories.len()
        );
        for cat in &lib.categories {
            assert!(!cat.commands.is_empty(), "{} has no commands", cat.id);
            for cmd in &cat.commands {
                assert!(!cmd.template.is_empty(), "{}: empty template", cmd.id);
                assert!(!cmd.title.is_empty(), "{}: empty title", cmd.id);
            }
        }
    }

    #[test]
    fn referenced_helpers_all_exist() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("commands");
        let lib = CommandLibrary::load(&[dir.as_path()]).expect("load library");
        let registry = crate::render::helpers::registry();
        let mut missing = Vec::new();
        for cat in &lib.categories {
            for cmd in &cat.commands {
                for name in extract_helper_names(&cmd.template) {
                    if !registry.contains_key(name.as_str()) {
                        missing.push(format!("{} -> ${{fn:{}(..)}}", cmd.id, name));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "missing helpers: {:#?}", missing);
    }

    #[test]
    fn when_conditions_parse() {
        // Render context with a Plaintext profile so all common atoms resolve.
        let mut ctx = crate::render::RenderContext::default();
        ctx.profile = Some(crate::engagement::CredentialProfile {
            name: "t".into(),
            username: "u".into(),
            domain: Some("D".into()),
            kind: crate::engagement::CredKind::Plaintext,
            password: Some("p".into()),
            nt_hash: None,
            ticket_path: None,
            notes: None,
        });
        ctx.target = Some(crate::engagement::Target {
            name: "x".into(),
            ip: Some("1.1.1.1".into()),
            hostname: Some("x.local".into()),
            dc_name: Some("DC".into()),
            lhost: Some("10.10.14.5".into()),
            lport: Some(4444),
            notes: None,
        });
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("commands");
        let lib = CommandLibrary::load(&[dir.as_path()]).expect("load library");
        for cat in &lib.categories {
            for cmd in &cat.commands {
                if let Some(when) = &cmd.when {
                    // smoke: just make sure evaluator runs (it's fail-open if it fails to parse).
                    let _ = crate::render::condition::evaluate(when, &ctx);
                }
            }
        }
    }

    fn extract_helper_names(template: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = template;
        while let Some(idx) = rest.find("${fn:") {
            let after = &rest[idx + 5..];
            if let Some(lp) = after.find('(') {
                names.push(after[..lp].trim().to_string());
            }
            rest = &rest[idx + 1..];
        }
        names
    }
}

fn merge_into(cats: &mut Vec<Category>, file: CategoryFile, source: PathBuf) {
    let cat = match cats.iter_mut().find(|c| c.id == file.category) {
        Some(c) => c,
        None => {
            cats.push(Category {
                id: file.category.clone(),
                display_name: file.display_name.clone().unwrap_or_else(|| file.category.clone()),
                icon: file.icon.clone(),
                order: file.order.unwrap_or(100),
                commands: Vec::new(),
                sources: Vec::new(),
            });
            cats.last_mut().unwrap()
        }
    };
    if let Some(dn) = file.display_name {
        cat.display_name = dn;
    }
    if let Some(icon) = file.icon {
        cat.icon = Some(icon);
    }
    if let Some(ord) = file.order {
        cat.order = ord;
    }
    cat.sources.push(source);
    for cmd in file.command {
        // Later entries with the same id REPLACE earlier ones (so engagement overrides win).
        if let Some(slot) = cat.commands.iter_mut().find(|c| c.id == cmd.id) {
            *slot = cmd;
        } else {
            cat.commands.push(cmd);
        }
    }
}
