//! Judge-brief protocol tests — ISC-93..97: brief shape, judgepack-as-data,
//! merge validation, score immutability, and render labeling.

use aispekt_core::judge::{content_hash, emit_brief, merge_brief, render_semantic, JUDGEPACK};
use aispekt_core::{analyze, AnalysisInput, RepoContext};
use serde_json::{json, Value};

const CONTENT: &str = "# Project\n\nAlways write clean code.\n\nRun `bun test` before committing.\n";

fn file_input() -> AnalysisInput {
    AnalysisInput {
        file_name: "AGENTS.md".into(),
        content: CONTENT.into(),
        repo: None,
    }
}

fn repo_input() -> AnalysisInput {
    AnalysisInput {
        file_name: "AGENTS.md".into(),
        content: CONTENT.into(),
        repo: Some(RepoContext {
            paths: vec!["AGENTS.md".into(), "package.json".into(), "src/main.ts".into()],
            ..Default::default()
        }),
    }
}

fn brief_for(input: &AnalysisInput) -> Value {
    let report = analyze(input);
    emit_brief(input, &report, "aispekt test")
}

/// Answers covering every task in the brief with empty findings.
fn all_clear_answers(brief: &Value) -> Value {
    let answers: Vec<Value> = brief["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({ "taskId": t["id"], "findings": [] }))
        .collect();
    json!({
        "protocol": "aispekt-judge/1",
        "contentHash": brief["target"]["contentHash"],
        "judge": { "agent": "test-agent", "model": "test-model" },
        "answers": answers,
    })
}

fn valid_finding() -> Value {
    json!({
        "lines": [3, 3],
        "quote": "Always write clean code.",
        "rationale": "Restates default behavior; no agent behaves differently without it.",
        "confidence": "high",
        "suggestion": "Delete the line."
    })
}

// ── judgepack (ISC-94) ────────────────────────────────────────────────────

#[test]
fn judgepack_loads_and_every_check_carries_question_and_evidence() {
    assert!(!JUDGEPACK.checks.is_empty());
    for c in &JUDGEPACK.checks {
        assert!(!c.question.trim().is_empty(), "{} question", c.id);
        assert!(!c.evidence_url.trim().is_empty(), "{} evidence", c.id);
        assert!(c.scope == "file" || c.scope == "repo", "{} scope", c.id);
    }
}

// ── brief shape (ISC-93) ──────────────────────────────────────────────────

#[test]
fn file_mode_brief_embeds_content_and_omits_repo_tasks() {
    let brief = brief_for(&file_input());
    assert_eq!(brief["protocol"], "aispekt-judge/1");
    assert_eq!(brief["target"]["mode"], "file");
    assert_eq!(brief["content"], CONTENT);
    assert!(brief["target"]["contentHash"]
        .as_str()
        .unwrap()
        .starts_with("fnv1a64:"));
    let task_ids: Vec<&str> = brief["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    let repo_checks: Vec<&str> = JUDGEPACK
        .checks
        .iter()
        .filter(|c| c.scope == "repo")
        .map(|c| c.id.as_str())
        .collect();
    for rc in repo_checks {
        assert!(!task_ids.contains(&rc), "repo check {rc} leaked into file mode");
    }
    assert!(brief.get("repoPaths").is_none());
    // deterministic context mirrors the engine's report
    let report = analyze(&file_input());
    assert_eq!(brief["deterministic"]["score"], json!(report.score));
    assert_eq!(brief["deterministic"]["grade"], json!(report.grade));
}

#[test]
fn repo_mode_brief_includes_repo_tasks_and_paths() {
    let brief = brief_for(&repo_input());
    assert_eq!(brief["target"]["mode"], "repo");
    let task_ids: Vec<&str> = brief["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(task_ids.contains(&"stale-guidance"));
    assert_eq!(brief["repoPaths"].as_array().unwrap().len(), 3);
    assert_eq!(brief["repoPathsTruncated"], json!(false));
}

#[test]
fn brief_is_deterministic() {
    let a = serde_json::to_string(&brief_for(&file_input())).unwrap();
    let b = serde_json::to_string(&brief_for(&file_input())).unwrap();
    assert_eq!(a, b);
}

#[test]
fn content_hash_is_stable_and_content_sensitive() {
    assert_eq!(content_hash("abc"), content_hash("abc"));
    assert_ne!(content_hash("abc"), content_hash("abd"));
}

// ── merge validation (ISC-95) ─────────────────────────────────────────────

#[test]
fn merge_happy_path_all_clear() {
    let brief = brief_for(&file_input());
    let merged = merge_brief(&brief, &all_clear_answers(&brief)).unwrap();
    for r in merged["semantic"]["results"].as_array().unwrap() {
        assert_eq!(r["status"], "clear");
    }
    assert!(merged["semantic"]["coverageWarnings"].as_array().unwrap().is_empty());
    assert_eq!(merged["semantic"]["judge"]["agent"], "test-agent");
}

#[test]
fn merge_flags_findings() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["answers"][0]["findings"] = json!([valid_finding()]);
    let merged = merge_brief(&brief, &answers).unwrap();
    assert_eq!(merged["semantic"]["results"][0]["status"], "flagged");
}

#[test]
fn merge_rejects_hash_mismatch() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["contentHash"] = json!("fnv1a64:0000000000000000");
    let err = merge_brief(&brief, &answers).unwrap_err();
    assert!(err.contains("content hash mismatch"), "err: {err}");
}

#[test]
fn merge_rejects_unknown_task_id() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["answers"][0]["taskId"] = json!("not-a-real-check");
    let err = merge_brief(&brief, &answers).unwrap_err();
    assert!(err.contains("unknown taskId"), "err: {err}");
}

#[test]
fn merge_rejects_duplicate_answers() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    let first = answers["answers"][0].clone();
    answers["answers"].as_array_mut().unwrap().push(first);
    let err = merge_brief(&brief, &answers).unwrap_err();
    assert!(err.contains("duplicate"), "err: {err}");
}

#[test]
fn merge_rejects_bad_confidence_and_empty_quote() {
    let brief = brief_for(&file_input());
    for (key, value) in [("confidence", json!("certain")), ("quote", json!("  "))] {
        let mut answers = all_clear_answers(&brief);
        let mut finding = valid_finding();
        finding[key] = value;
        answers["answers"][0]["findings"] = json!([finding]);
        assert!(merge_brief(&brief, &answers).is_err(), "accepted bad {key}");
    }
}

#[test]
fn merge_rejects_wrong_protocol() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["protocol"] = json!("aispekt-judge/999");
    assert!(merge_brief(&brief, &answers).is_err());
}

#[test]
fn unanswered_tasks_warn_but_do_not_fail() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["answers"].as_array_mut().unwrap().remove(0);
    let merged = merge_brief(&brief, &answers).unwrap();
    let warnings = merged["semantic"]["coverageWarnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert_eq!(merged["semantic"]["results"][0]["status"], "unanswered");
}

// ── score immutability (ISC-96, Anti) ─────────────────────────────────────

#[test]
fn adversarial_answers_never_move_the_deterministic_score() {
    let input = file_input();
    let report = analyze(&input);
    let brief = emit_brief(&input, &report, "aispekt test");

    // Adversarial: answers file smuggles score-shaped fields everywhere.
    let mut answers = all_clear_answers(&brief);
    answers["score"] = json!(0);
    answers["deterministic"] = json!({ "score": 0, "grade": "F" });
    answers["answers"][0]["score"] = json!(0);
    let merged = merge_brief(&brief, &answers).unwrap();
    assert_eq!(merged["deterministic"], brief["deterministic"]);
    assert_eq!(merged["deterministic"]["score"], json!(report.score));

    // Pseudo-randomized sweep: arbitrary finding content, same invariant.
    let mut seed: u64 = 0x5eed;
    for _ in 0..100 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (seed >> 33) % 5;
        let conf = ["high", "medium", "low"][(seed % 3) as usize];
        let findings: Vec<Value> = (0..n)
            .map(|k| {
                json!({
                    "lines": [1 + k, 2 + k],
                    "quote": format!("q{seed:x}{k}"),
                    "rationale": format!("r{seed:x}"),
                    "confidence": conf,
                })
            })
            .collect();
        let mut a = all_clear_answers(&brief);
        a["answers"][(seed % 3) as usize]["findings"] = Value::Array(findings);
        let merged = merge_brief(&brief, &a).unwrap();
        assert_eq!(merged["deterministic"], brief["deterministic"]);
    }
}

// ── render labeling (ISC-97) ──────────────────────────────────────────────

#[test]
fn render_separates_and_labels_semantic_output() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["answers"][0]["findings"] = json!([valid_finding()]);
    let merged = merge_brief(&brief, &answers).unwrap();
    let text = render_semantic(&merged);
    assert!(text.contains("aispekt semantic judge"));
    assert!(text.contains("unchanged; semantic verdicts never affect it"));
    assert!(text.contains("judge: test-agent (test-model)"));
    assert!(text.contains("non-deterministic"));
    assert!(text.contains("⚑"));
    assert!(text.contains("✓"));
    assert!(text.contains("L3 (high)"));
}
