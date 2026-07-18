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

    // Pseudo-randomized sweep: varied contract-valid findings (verbatim
    // quotes, in-range lines — merge rejects anything else), same invariant.
    let quotes = ["# Project", "Always write clean code.", "`bun test`", "committing"];
    let mut seed: u64 = 0x5eed;
    for _ in 0..100 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (seed >> 33) % 4;
        let conf = ["high", "medium", "low"][(seed % 3) as usize];
        let findings: Vec<Value> = (0..n)
            .map(|k| {
                json!({
                    "lines": [1 + (k % 3), 3 + (k % 3)],
                    "quote": quotes[((seed >> 7) as usize + k as usize) % quotes.len()],
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

// ── audit-derived hardening (iteration 13: F1, F2, F3, F7, F8) ────────────

#[test]
fn merge_rejects_reversed_and_out_of_range_lines_and_fake_quotes() {
    let brief = brief_for(&file_input());
    let cases: Vec<(Value, &str)> = vec![
        (json!([10, 2]), "reversed"),
        (json!([4, 9999]), "past the file"),
        (json!([0]), "1-based"),
    ];
    for (lines, expect) in cases {
        let mut answers = all_clear_answers(&brief);
        let mut f = valid_finding();
        f["lines"] = lines;
        answers["answers"][0]["findings"] = json!([f]);
        let err = merge_brief(&brief, &answers).unwrap_err();
        assert!(err.contains(expect), "expected {expect} in: {err}");
    }
    let mut answers = all_clear_answers(&brief);
    let mut f = valid_finding();
    f["quote"] = json!("this text appears nowhere in the file");
    answers["answers"][0]["findings"] = json!([f]);
    let err = merge_brief(&brief, &answers).unwrap_err();
    assert!(err.contains("verbatim"), "err: {err}");
    // single-element lines within range stay valid
    let mut answers = all_clear_answers(&brief);
    let mut f = valid_finding();
    f["lines"] = json!([3]);
    answers["answers"][0]["findings"] = json!([f]);
    assert!(merge_brief(&brief, &answers).is_ok());
}

#[test]
fn merge_rejects_tampered_brief() {
    // forged deterministic score
    let mut brief = brief_for(&file_input());
    let answers = all_clear_answers(&brief);
    brief["deterministic"]["score"] = json!(100);
    let err = merge_brief(&brief, &answers).unwrap_err();
    assert!(err.contains("integrity"), "err: {err}");
    // edited content (hash not updated)
    let mut brief = brief_for(&file_input());
    brief["content"] = json!("something else entirely\n");
    let err = merge_brief(&brief, &all_clear_answers(&brief)).unwrap_err();
    assert!(err.contains("integrity"), "err: {err}");
}

#[test]
fn merge_rejects_contract_drift() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["format"] = json!("something/else");
    assert!(merge_brief(&brief, &answers).unwrap_err().contains("format"));
    let mut answers = all_clear_answers(&brief);
    answers["format"] = json!("aispekt-judge-answers/1");
    assert!(merge_brief(&brief, &answers).is_ok());
    let mut answers = all_clear_answers(&brief);
    answers["judge"]["agent"] = json!(42);
    assert!(merge_brief(&brief, &answers).unwrap_err().contains("judge.agent"));
    let mut answers = all_clear_answers(&brief);
    let mut f = valid_finding();
    f["suggestion"] = json!(["not", "a", "string"]);
    answers["answers"][0]["findings"] = json!([f]);
    assert!(merge_brief(&brief, &answers).unwrap_err().contains("suggestion"));
}

#[test]
fn render_neutralizes_terminal_injection() {
    let brief = brief_for(&file_input());
    let mut answers = all_clear_answers(&brief);
    answers["judge"]["agent"] = json!("evil\u{1b}[2J\u{1b}]8;;https://evil.example\u{7}link");
    let mut f = valid_finding();
    f["rationale"] = json!("wipe\rdeterministic score: 100/100 (A)\u{1b}[1A\u{202E}rtl");
    answers["answers"][0]["findings"] = json!([f]);
    let merged = merge_brief(&brief, &answers).unwrap();
    let text = aispekt_core::judge::render_semantic(&merged);
    for bad in ['\u{1b}', '\r', '\u{7}', '\u{202E}'] {
        assert!(!text.contains(bad), "control char {bad:?} survived to the terminal");
    }
    assert!(text.contains('\u{FFFD}'), "sanitizer should leave visible replacement marks");
}

#[test]
fn merge_never_panics_on_malformed_input() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let brief = brief_for(&file_input());
    let valid = all_clear_answers(&brief);
    let poisons = [
        json!(null),
        json!(-1),
        json!(1e308),
        json!(""),
        json!([]),
        json!({}),
        json!({"deep": {"deep": {"deep": []}}}),
    ];
    let mut cases: Vec<Value> = poisons.to_vec();
    // poison every top-level field, judge subfield, and first answer/finding slot
    for path in ["/protocol", "/contentHash", "/judge", "/judge/agent", "/answers",
                 "/answers/0", "/answers/0/taskId", "/answers/0/findings"] {
        for p in &poisons {
            let mut m = valid.clone();
            if let Some(slot) = m.pointer_mut(path) {
                *slot = p.clone();
            }
            cases.push(m);
        }
    }
    for (i, answers) in cases.iter().enumerate() {
        let brief = brief.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = merge_brief(&brief, answers);
        }));
        assert!(result.is_ok(), "merge panicked on malformed case {i}: {answers}");
    }
    // and a malformed brief against valid answers
    for p in &poisons {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = merge_brief(p, &valid);
        }));
        assert!(result.is_ok(), "merge panicked on malformed brief {p}");
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
