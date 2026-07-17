use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        }
    }
    pub fn order(&self) -> u8 {
        match self {
            Severity::Error => 0,
            Severity::Warn => 1,
            Severity::Info => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceTier {
    Measured,
    Official,
    Community,
    Heuristic,
}

impl EvidenceTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceTier::Measured => "measured",
            EvidenceTier::Official => "official",
            EvidenceTier::Community => "community",
            EvidenceTier::Heuristic => "heuristic",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMeta {
    pub id: String,
    pub name: String,
    pub severity: Severity,
    pub weight: f64,
    pub max_penalty: f64,
    pub evidence_tier: EvidenceTier,
    pub evidence_url: String,
    pub last_verified: String,
    pub summary: String,
    pub recommendation: String,
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RulePack {
    pub version: String,
    pub updated: String,
    pub rules: Vec<RuleMeta>,
}

/// Insertion-ordered string map — the TS engine iterates `Object.keys()` in
/// insertion order, and both determinism and lookup-precedence parity depend
/// on preserving it (HashMap iteration order is randomized per process).
#[derive(Debug, Clone, Default)]
pub struct ContentMap(pub Vec<(String, String)>);

impl ContentMap {
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
    pub fn insert(&mut self, key: String, value: String) {
        self.0.push((key, value));
    }
}

impl<'de> Deserialize<'de> for ContentMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = ContentMap;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of file name to file content")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    out.push((k, v));
                }
                Ok(ContentMap(out))
            }
        }
        deserializer.deserialize_map(V)
    }
}

/// Repo context: full path list plus contents of the files checks need.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RepoContext {
    pub paths: Vec<String>,
    #[serde(default)]
    pub contents: ContentMap,
    /// Path enumeration hit the cap — absence is no longer provable.
    #[serde(default)]
    pub truncated: bool,
    /// Browser folder inputs omit symlinks entirely.
    #[serde(default, rename = "symlinksHidden")]
    pub symlinks_hidden: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisInput {
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub content: String,
    #[serde(default)]
    pub repo: Option<RepoContext>,
}

/// What a check implementation returns; metadata is joined from the rulepack.
#[derive(Debug, Clone)]
pub struct RawFinding {
    pub line: usize,
    pub excerpt: String,
    pub message: String,
    /// Overrides the rule's generic recommendation when the case needs its own fix text.
    pub recommendation: Option<String>,
}

impl RawFinding {
    pub fn new(line: usize, excerpt: impl Into<String>, message: impl Into<String>) -> Self {
        RawFinding {
            line,
            excerpt: excerpt.into(),
            message: message.into(),
            recommendation: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub line: usize,
    pub excerpt: String,
    pub message: String,
    /// Set when the raw finding carried its own recommendation — the TS engine
    /// emits that key in spread position, and JSON parity must reproduce it.
    pub raw_recommendation: bool,
    pub rule_id: String,
    pub rule_name: String,
    pub severity: Severity,
    pub recommendation: String,
    pub evidence_url: String,
    pub evidence_tier: EvidenceTier,
}

#[derive(Debug, Clone)]
pub struct RulePenalty {
    pub rule_id: String,
    pub count: usize,
    pub penalty: f64,
}

#[derive(Debug, Clone)]
pub struct AgentNote {
    pub agent: String,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub file_name: String,
    pub mode: &'static str,
    pub score: i64,
    pub grade: &'static str,
    pub line_count: usize,
    pub byte_size: usize,
    pub token_estimate: u64,
    pub findings: Vec<Finding>,
    pub penalties: Vec<RulePenalty>,
    pub agent_notes: Vec<AgentNote>,
    /// Analysis-quality caveats (e.g. truncated walk) — never silent.
    pub notices: Vec<String>,
    pub rulepack_version: String,
    pub rulepack_updated: String,
}

#[derive(Debug, Clone)]
pub struct CodeSpan {
    pub line: usize,
    pub text: String,
}

/// Shared parsed view of the analyzed file, passed to every check.
#[derive(Debug, Clone)]
pub struct CheckContext {
    pub file_name: String,
    pub content: String,
    pub lines: Vec<String>,
    /// true where the line is prose (outside fenced code blocks)
    pub prose: Vec<bool>,
    /// all inline code spans and fenced block lines, with their line numbers
    pub code_spans: Vec<CodeSpan>,
    pub repo: Option<RepoContext>,
}

pub type CheckFn = fn(&CheckContext) -> Vec<RawFinding>;
