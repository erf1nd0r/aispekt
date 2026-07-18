//! Judge-brief protocol (`aispekt-judge/1`) — the semantic layer's seam.
//!
//! The deterministic engine scores what regexes can prove. Questions only a
//! model can answer (dead weight, semantic vagueness, contradictions, stale
//! guidance) are emitted as a self-contained JSON *brief*; any judge — an
//! agent running the shipped skill, a BYOK script, a human — completes it,
//! and `merge` validates the answers and renders them as a clearly separate
//! semantic report. Two invariants hold everywhere:
//!
//! 1. Judge output NEVER touches the deterministic score.
//! 2. Answers bind to the exact content they judged (fnv1a64 content hash —
//!    integrity against stale/mismatched answers, not a security boundary).
//!
//! Like the engine, everything here is pure: no I/O, callers own the
//! filesystem. Checks are data in rules/judgepack.json, mirroring the
//! rulepack doctrine: adding a semantic check is a data edit.

use crate::types::{AnalysisInput, Report};
use serde_json::{json, Map, Value};
use std::fmt::Write as _;
use std::sync::LazyLock;

pub const PROTOCOL: &str = "aispekt-judge/1";
pub const ANSWERS_FORMAT: &str = "aispekt-judge-answers/1";
/// Repo-mode briefs embed at most this many paths (BFS order, roots first).
pub const MAX_BRIEF_PATHS: usize = 500;

pub const JUDGEPACK_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../rules/judgepack.json"));

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeCheck {
    pub id: String,
    pub name: String,
    pub question: String,
    pub guidance: String,
    pub evidence_tier: String,
    pub evidence_url: String,
    pub last_verified: String,
    pub scope: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct JudgePack {
    pub version: String,
    pub protocol: String,
    pub updated: String,
    pub checks: Vec<JudgeCheck>,
}

pub static JUDGEPACK: LazyLock<JudgePack> = LazyLock::new(|| {
    let pack: JudgePack =
        serde_json::from_str(JUDGEPACK_JSON).expect("judgepack.json is invalid");
    assert_eq!(pack.protocol, PROTOCOL, "judgepack protocol mismatch");
    for c in &pack.checks {
        assert!(!c.question.trim().is_empty(), "judge check {} has no question", c.id);
        assert!(!c.evidence_url.trim().is_empty(), "judge check {} has no evidence", c.id);
        assert!(c.scope == "file" || c.scope == "repo", "judge check {} bad scope", c.id);
    }
    pack
});

/// FNV-1a 64-bit over the raw bytes, prefixed so the algorithm is explicit.
pub fn content_hash(content: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{h:016x}")
}

/// Build the self-contained brief. Pure; deterministic for identical input.
pub fn emit_brief(input: &AnalysisInput, report: &Report, generator: &str) -> Value {
    let repo_mode = input.repo.is_some();
    let tasks: Vec<Value> = JUDGEPACK
        .checks
        .iter()
        .filter(|c| c.scope == "file" || repo_mode)
        .map(|c| {
            json!({
                "id": c.id,
                "name": c.name,
                "question": c.question,
                "guidance": c.guidance,
                "evidenceUrl": c.evidence_url,
                "evidenceTier": c.evidence_tier,
                "scope": c.scope,
            })
        })
        .collect();
    let hash = content_hash(&input.content);
    let mut brief = json!({
        "protocol": PROTOCOL,
        "generator": generator,
        "judgepackVersion": JUDGEPACK.version,
        "target": {
            "fileName": input.file_name,
            "mode": if repo_mode { "repo" } else { "file" },
            "lineCount": report.line_count,
            "contentHash": hash,
        },
        "deterministic": {
            "score": report.score,
            "grade": report.grade,
            "rulepackVersion": report.rulepack_version,
            "findingCount": report.findings.len(),
            "note": "Computed by the deterministic engine. Semantic verdicts never change it.",
        },
        "tasks": tasks,
        "content": input.content,
        "answersContract": {
            "format": ANSWERS_FORMAT,
            "shape": {
                "protocol": PROTOCOL,
                "contentHash": "<copy target.contentHash verbatim>",
                "judge": { "agent": "<what you are, e.g. claude-code>", "model": "<model id, or unknown>" },
                "answers": [{
                    "taskId": "<a task id from this brief>",
                    "findings": [{
                        "lines": [1, 1],
                        "quote": "<verbatim excerpt from content>",
                        "rationale": "<why this fails the task's question>",
                        "confidence": "high|medium|low",
                        "suggestion": "<concrete fix, optional>",
                    }],
                }],
            },
            "rules": [
                "Answer every task; an empty findings array means the check passes.",
                "Judge only the embedded content; quotes must be verbatim.",
                "Never invent line numbers; lines is [start, end], 1-based.",
                "The deterministic score is not yours to change or restate.",
            ],
        },
    });
    if let Some(repo) = &input.repo {
        let obj = brief.as_object_mut().expect("brief is an object");
        let paths: Vec<Value> =
            repo.paths.iter().take(MAX_BRIEF_PATHS).map(|p| Value::from(p.as_str())).collect();
        let truncated = repo.truncated || repo.paths.len() > MAX_BRIEF_PATHS;
        obj.insert("repoPaths".into(), Value::Array(paths));
        obj.insert("repoPathsTruncated".into(), Value::from(truncated));
    }
    brief
}

fn field<'a>(v: &'a Value, key: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("missing field \"{key}\""))
}

fn str_field(v: &Value, key: &str) -> Result<String, String> {
    field(v, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("field \"{key}\" must be a string"))
}

fn validate_finding(f: &Value, idx: usize, task_id: &str) -> Result<(), String> {
    let at = format!("answers[{task_id}].findings[{idx}]");
    let obj = f.as_object().ok_or_else(|| format!("{at} must be an object"))?;
    let lines = obj.get("lines").ok_or_else(|| format!("{at}.lines missing"))?;
    let arr = lines.as_array().ok_or_else(|| format!("{at}.lines must be an array"))?;
    if arr.is_empty() || arr.len() > 2 || !arr.iter().all(|n| n.as_u64().is_some_and(|n| n >= 1)) {
        return Err(format!("{at}.lines must be [start] or [start, end], 1-based"));
    }
    for key in ["quote", "rationale"] {
        let s = obj.get(key).and_then(Value::as_str).unwrap_or("");
        if s.trim().is_empty() {
            return Err(format!("{at}.{key} must be a non-empty string"));
        }
    }
    let conf = obj.get("confidence").and_then(Value::as_str).unwrap_or("");
    if !matches!(conf, "high" | "medium" | "low") {
        return Err(format!("{at}.confidence must be high|medium|low"));
    }
    Ok(())
}

/// Validate answers against the brief and produce the merged report.
/// The merged report's `deterministic` block is copied from the brief
/// untouched — by construction the judge cannot move the score.
pub fn merge_brief(brief: &Value, answers: &Value) -> Result<Value, String> {
    if str_field(brief, "protocol")? != PROTOCOL {
        return Err(format!("brief protocol is not {PROTOCOL}"));
    }
    if str_field(answers, "protocol")? != PROTOCOL {
        return Err(format!("answers protocol is not {PROTOCOL}"));
    }
    let target = field(brief, "target")?;
    let brief_hash = str_field(target, "contentHash")?;
    let answers_hash = str_field(answers, "contentHash")?;
    if brief_hash != answers_hash {
        return Err(format!(
            "content hash mismatch: brief has {brief_hash}, answers have {answers_hash} — \
             the answers were made against different content; re-run judge emit"
        ));
    }
    let tasks = field(brief, "tasks")?
        .as_array()
        .ok_or("brief tasks must be an array")?;
    let task_ids: Vec<String> =
        tasks.iter().map(|t| str_field(t, "id")).collect::<Result<_, _>>()?;
    let judge = field(answers, "judge")?;
    let judge_agent = str_field(judge, "agent").unwrap_or_else(|_| "unknown".into());
    let judge_model = str_field(judge, "model").unwrap_or_else(|_| "unknown".into());

    let answer_list = field(answers, "answers")?
        .as_array()
        .ok_or("answers.answers must be an array")?;
    let mut by_task: Map<String, Value> = Map::new();
    for a in answer_list {
        let tid = str_field(a, "taskId")?;
        if !task_ids.contains(&tid) {
            return Err(format!("unknown taskId \"{tid}\" — not in the brief"));
        }
        if by_task.contains_key(&tid) {
            return Err(format!("duplicate answer for taskId \"{tid}\""));
        }
        let findings = field(a, "findings")?
            .as_array()
            .ok_or_else(|| format!("answers[{tid}].findings must be an array"))?;
        for (i, f) in findings.iter().enumerate() {
            validate_finding(f, i, &tid)?;
        }
        by_task.insert(tid, Value::Array(findings.clone()));
    }
    let unanswered: Vec<String> =
        task_ids.iter().filter(|id| !by_task.contains_key(*id)).cloned().collect();

    let results: Vec<Value> = tasks
        .iter()
        .map(|t| {
            let id = t.get("id").and_then(Value::as_str).unwrap_or_default();
            let findings = by_task.get(id).cloned();
            json!({
                "taskId": id,
                "name": t.get("name").cloned().unwrap_or_default(),
                "evidenceUrl": t.get("evidenceUrl").cloned().unwrap_or_default(),
                "evidenceTier": t.get("evidenceTier").cloned().unwrap_or_default(),
                "status": match &findings {
                    None => "unanswered",
                    Some(Value::Array(a)) if a.is_empty() => "clear",
                    Some(_) => "flagged",
                },
                "findings": findings.unwrap_or_else(|| Value::Array(vec![])),
            })
        })
        .collect();

    Ok(json!({
        "protocol": PROTOCOL,
        "target": target.clone(),
        "deterministic": field(brief, "deterministic")?.clone(),
        "semantic": {
            "judge": { "agent": judge_agent, "model": judge_model },
            "caveat": "Semantic verdicts are model judgments: non-deterministic, judge-dependent, and never part of the deterministic score.",
            "results": results,
            "coverageWarnings": unanswered.iter()
                .map(|id| format!("task \"{id}\" was not answered"))
                .collect::<Vec<_>>(),
        },
    }))
}

/// Human rendering of a merged report. The deterministic score is shown
/// exactly as the engine computed it, labeled unchanged.
pub fn render_semantic(merged: &Value) -> String {
    let mut out = String::new();
    let get = |path: &[&str]| -> String {
        let mut v = merged;
        for k in path {
            v = v.get(k).unwrap_or(&Value::Null);
        }
        match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    };
    let _ = writeln!(out, "aispekt semantic judge — {}", get(&["target", "fileName"]));
    let _ = writeln!(
        out,
        "deterministic score: {}/100 ({}) — unchanged; semantic verdicts never affect it",
        get(&["deterministic", "score"]),
        get(&["deterministic", "grade"]),
    );
    let _ = writeln!(
        out,
        "judge: {} ({})",
        get(&["semantic", "judge", "agent"]),
        get(&["semantic", "judge", "model"]),
    );
    let _ = writeln!(out, "caveat: {}", get(&["semantic", "caveat"]));
    let empty = vec![];
    let results = merged
        .pointer("/semantic/results")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    for r in results {
        let name = r.get("name").and_then(Value::as_str).unwrap_or("?");
        let status = r.get("status").and_then(Value::as_str).unwrap_or("?");
        let findings = r.get("findings").and_then(Value::as_array).cloned().unwrap_or_default();
        let _ = writeln!(out);
        match status {
            "clear" => {
                let _ = writeln!(out, "✓ {name} — clear");
            }
            "unanswered" => {
                let _ = writeln!(out, "· {name} — unanswered");
            }
            _ => {
                let _ = writeln!(out, "⚑ {name} — {} flagged", findings.len());
            }
        }
        for f in &findings {
            let lines = f.get("lines").and_then(Value::as_array).cloned().unwrap_or_default();
            let l = match lines.as_slice() {
                [a] => format!("L{a}"),
                [a, b] if a == b => format!("L{a}"),
                [a, b] => format!("L{a}-{b}"),
                _ => "L?".into(),
            };
            let conf = f.get("confidence").and_then(Value::as_str).unwrap_or("?");
            // Multi-line quotes keep their alignment under the marker line.
            let quote = f
                .get("quote")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace('\n', "\n           ");
            let rationale = f.get("rationale").and_then(Value::as_str).unwrap_or("");
            let _ = writeln!(out, "  {l} ({conf}) \"{quote}\"");
            let _ = writeln!(out, "    {rationale}");
            if let Some(s) = f.get("suggestion").and_then(Value::as_str) {
                if !s.trim().is_empty() {
                    let _ = writeln!(out, "    fix: {s}");
                }
            }
        }
    }
    let warnings = merged
        .pointer("/semantic/coverageWarnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !warnings.is_empty() {
        let _ = writeln!(out);
        for w in &warnings {
            let _ = writeln!(out, "⚠ {}", w.as_str().unwrap_or_default());
        }
    }
    out
}
