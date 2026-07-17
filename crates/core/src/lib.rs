//! aispekt-core — evidence-cited analyzer for AGENTS.md / CLAUDE.md agent
//! instruction files. Pure, deterministic, no I/O: callers (CLI, WASM) own
//! the filesystem and the browser.
//!
//! This is a semantics-preserving port of the original TypeScript engine;
//! its JSON report is byte-compared against that engine's output in
//! tests/golden.rs. Behavioral quirks (spread-position key order, UTF-16
//! length semantics, JS rounding) are reproduced deliberately.

pub mod analyze;
pub mod checks {
    pub mod file;
    pub mod repo;
}
pub mod parse;
pub mod render;
pub mod rulepack;
pub mod types;

pub use analyze::{analyze, report_to_json, report_to_json_pretty, report_to_value};
pub use parse::strip_common_root;
pub use rulepack::{RULEPACK, RULEPACK_JSON};
pub use types::{AnalysisInput, ContentMap, RepoContext, Report};
