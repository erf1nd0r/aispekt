use crate::types::{CheckFn, RulePack};
use regex::Regex;
use std::sync::LazyLock;

/// The rulepack is data, not code: rules/rulepack.json is the single source of
/// truth for weights, severities, and evidence, embedded at compile time.
pub const RULEPACK_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../rules/rulepack.json"));

pub static RULEPACK: LazyLock<RulePack> = LazyLock::new(|| {
    let pack: RulePack = serde_json::from_str(RULEPACK_JSON).expect("rulepack.json is invalid");
    validate_pack(&pack, IMPLS);
    pack
});

pub const IMPLS: &[(&str, CheckFn)] = &[
    ("lint-leakage", crate::checks::file::lint_leakage),
    ("directory-tree", crate::checks::file::directory_tree),
    ("vague-rules", crate::checks::file::vague_rules),
    ("no-commands", crate::checks::file::no_commands),
    ("enforcement-prose", crate::checks::file::enforcement_prose),
    ("secret-leak", crate::checks::file::secret_leak),
    ("emphasis-overuse", crate::checks::file::emphasis_overuse),
    (
        "pointer-without-instruction",
        crate::checks::file::pointer_without_instruction,
    ),
    ("size-cost", crate::checks::file::size_cost),
    ("duplicate-rules", crate::checks::file::duplicate_rules),
    ("contradiction-lite", crate::checks::file::contradiction_lite),
    ("boundaries-missing", crate::checks::file::boundaries_missing),
    ("codex-cap", crate::checks::file::codex_cap),
    ("dead-command", crate::checks::repo::dead_command),
    ("stale-path", crate::checks::repo::stale_path),
    ("readme-redundancy", crate::checks::repo::readme_redundancy),
    ("bridge-missing", crate::checks::repo::bridge_missing),
    ("import-resolution", crate::checks::repo::import_resolution),
];

pub fn impl_for(id: &str) -> Option<CheckFn> {
    IMPLS.iter().find(|(rid, _)| *rid == id).map(|(_, f)| *f)
}

/// Rulepack and implementations must agree exactly — a rule with no code and
/// code with no rule are both configuration drift, and drift panics.
pub fn validate_pack(pack: &RulePack, impls: &[(&str, CheckFn)]) {
    static HTTPS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https://").unwrap());
    for rule in &pack.rules {
        if impl_for(&rule.id).is_none() {
            panic!("rulepack rule \"{}\" has no implementation", rule.id);
        }
        // https-only is a security invariant, not a style rule: evidenceUrl
        // lands in an href, and a javascript:/data: value there would be XSS
        // if a rule pack ever sourced URLs from outside the bundle.
        if !HTTPS_RE.is_match(&rule.evidence_url) {
            panic!("rule \"{}\" evidenceUrl must be https://", rule.id);
        }
        if rule.weight <= 0.0 || rule.max_penalty <= 0.0 {
            panic!("rule \"{}\" has non-positive weight/maxPenalty", rule.id);
        }
    }
    for (id, _) in impls {
        if !pack.rules.iter().any(|r| r.id == *id) {
            panic!("implementation \"{id}\" is not in the rulepack");
        }
    }
}
