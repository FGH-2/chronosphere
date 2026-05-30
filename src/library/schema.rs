use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFile {
    pub category: String,
    pub display_name: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    #[serde(default)]
    pub command: Vec<CommandEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub id: String,
    pub title: String,
    pub template: String,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub variants: Vec<CommandVariant>,
    /// Where the command may run: `local` (default), `remote`, or `any`.
    #[serde(default = "default_execution_local")]
    pub execution: String,
}

fn default_execution_local() -> String {
    "local".into()
}

impl CommandEntry {
    /// Returns the variant whose `when` matches the given evaluator, else falls back to the base template.
    pub fn applicable_template<F: Fn(&str) -> bool>(&self, eval: &F) -> &str {
        for v in &self.variants {
            if eval(v.when.as_deref().unwrap_or("true")) {
                return &v.template;
            }
        }
        &self.template
    }

    pub fn is_applicable<F: Fn(&str) -> bool>(&self, eval: &F) -> bool {
        match &self.when {
            Some(w) => eval(w),
            None => true,
        }
    }

    pub fn allows_remote(&self) -> bool {
        matches!(self.execution.as_str(), "remote" | "any")
    }

    pub fn allows_local(&self) -> bool {
        matches!(self.execution.as_str(), "local" | "any" | "")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandVariant {
    pub when: Option<String>,
    pub template: String,
}
