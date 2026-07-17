use crate::parse::{build_context, estimate_tokens};
use crate::render::{js_round, to_fixed1, to_locale_string};
use crate::rulepack::{impl_for, RULEPACK};
use crate::types::{AgentNote, AnalysisInput, Finding, Report, RulePenalty};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::LazyLock;

fn grade_of(score: i64) -> &'static str {
    if score >= 90 {
        "A"
    } else if score >= 75 {
        "B"
    } else if score >= 60 {
        "C"
    } else if score >= 40 {
        "D"
    } else {
        "F"
    }
}

fn agent_notes(input: &AnalysisInput, line_count: usize, byte_size: usize) -> Vec<AgentNote> {
    let lower = input.file_name.to_lowercase();
    let is_claude = lower.ends_with("claude.md");
    let is_agents = lower.ends_with("agents.md");
    let kib = to_fixed1(byte_size as f64 / 1024.0);
    vec![
        AgentNote {
            agent: "Claude Code".to_string(),
            note: format!(
                "{}Injects {} as an advisory user message (no compliance guarantee); official size target is under 200 lines{}. Zero-exception rules belong in hooks.",
                if is_agents {
                    "Does not read AGENTS.md natively — needs a CLAUDE.md containing `@AGENTS.md`. "
                } else {
                    ""
                },
                if is_claude { "this file" } else { "CLAUDE.md" },
                if line_count > 200 {
                    format!(" — this file is {line_count}")
                } else {
                    String::new()
                }
            ),
        },
        AgentNote {
            agent: "OpenAI Codex".to_string(),
            note: if byte_size > 32 * 1024 {
                format!(
                    "Truncates at 32 KiB — this file is {kib} KiB, everything past the cap is silently dropped."
                )
            } else {
                format!(
                    "Reads AGENTS.md natively (nearest file wins, 32 KiB cap — this file is {kib} KiB). Supports AGENTS.override.md for local tweaks."
                )
            },
        },
        AgentNote {
            agent: "Cursor".to_string(),
            note: "Reads AGENTS.md natively. Path-scoped rules need .cursor/rules/*.mdc — plain .md files there are silently ignored.".to_string(),
        },
        AgentNote {
            agent: "GitHub Copilot".to_string(),
            note: "Reads AGENTS.md natively (coding agent, since Aug 2025) alongside .github/copilot-instructions.md.".to_string(),
        },
    ]
}

static IGNORE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(aispekt|prune)-ignore").unwrap());

pub fn analyze(input: &AnalysisInput) -> Report {
    let pack = &*RULEPACK;
    let ctx = build_context(&input.file_name, &input.content, input.repo.clone());
    let mode = if input.repo.is_some() { "repo" } else { "file" };

    let mut findings: Vec<Finding> = Vec::new();
    let mut penalties: Vec<RulePenalty> = Vec::new();
    let mut total_penalty = 0.0f64;

    // Escape hatch for heuristics: an `aispekt-ignore` marker (legacy:
    // prune-ignore) on the finding's line or the line above suppresses it.
    let is_ignored = |line: usize| -> bool {
        let at = |idx: usize| {
            idx.checked_sub(1)
                .and_then(|i| ctx.lines.get(i))
                .map(|l| IGNORE_RE.is_match(l))
                .unwrap_or(false)
        };
        at(line) || at(line.wrapping_sub(1))
    };

    for rule in &pack.rules {
        if rule.scope == "repo" && input.repo.is_none() {
            continue;
        }
        let check = impl_for(&rule.id)
            .unwrap_or_else(|| panic!("no implementation for rule \"{}\"", rule.id));
        let raw: Vec<_> = check(&ctx)
            .into_iter()
            .filter(|f| !is_ignored(f.line))
            .collect();
        if raw.is_empty() {
            continue;
        }
        let count = raw.len();
        for f in raw {
            findings.push(Finding {
                line: f.line,
                excerpt: f.excerpt,
                message: f.message,
                raw_recommendation: f.recommendation.is_some(),
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                severity: rule.severity,
                recommendation: f.recommendation.unwrap_or_else(|| rule.recommendation.clone()),
                evidence_url: rule.evidence_url.clone(),
                evidence_tier: rule.evidence_tier,
            });
        }
        let penalty = rule.max_penalty.min(count as f64 * rule.weight);
        penalties.push(RulePenalty {
            rule_id: rule.id.clone(),
            count,
            penalty,
        });
        total_penalty += penalty;
    }

    let score = js_round(100.0 - total_penalty).clamp(0.0, 100.0) as i64;
    let byte_size = input.content.len();

    findings.sort_by(|a, b| {
        a.severity
            .order()
            .cmp(&b.severity.order())
            .then(a.line.cmp(&b.line))
            .then(a.rule_id.cmp(&b.rule_id))
    });

    let mut notices: Vec<String> = Vec::new();
    if let Some(repo) = &input.repo {
        if repo.truncated {
            notices.push(format!(
                "Repo walk hit the {}-path cap — stale-path and @-import drift checks were skipped rather than run against a partial file list.",
                to_locale_string(repo.paths.len())
            ));
        }
    }

    Report {
        file_name: input.file_name.clone(),
        mode,
        score,
        grade: grade_of(score),
        line_count: ctx.lines.len(),
        byte_size,
        token_estimate: estimate_tokens(&input.content),
        findings,
        penalties,
        agent_notes: agent_notes(input, ctx.lines.len(), byte_size),
        notices,
        rulepack_version: pack.version.clone(),
        rulepack_updated: pack.updated.clone(),
    }
}

fn number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

/// Serialize a report exactly as the TS engine's `JSON.stringify(report)`
/// would: same key order, including the spread-position quirk where a raw
/// per-finding recommendation surfaces before ruleId.
pub fn report_to_value(report: &Report) -> Value {
    let findings: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            let mut m = Map::new();
            m.insert("line".into(), json!(f.line));
            m.insert("excerpt".into(), json!(f.excerpt));
            m.insert("message".into(), json!(f.message));
            if f.raw_recommendation {
                m.insert("recommendation".into(), json!(f.recommendation));
            }
            m.insert("ruleId".into(), json!(f.rule_id));
            m.insert("ruleName".into(), json!(f.rule_name));
            m.insert("severity".into(), json!(f.severity.as_str()));
            if !f.raw_recommendation {
                m.insert("recommendation".into(), json!(f.recommendation));
            }
            m.insert("evidenceUrl".into(), json!(f.evidence_url));
            m.insert("evidenceTier".into(), json!(f.evidence_tier.as_str()));
            Value::Object(m)
        })
        .collect();
    let penalties: Vec<Value> = report
        .penalties
        .iter()
        .map(|p| {
            json!({
                "ruleId": p.rule_id,
                "count": p.count,
                "penalty": number(p.penalty),
            })
        })
        .collect();
    let agent_notes: Vec<Value> = report
        .agent_notes
        .iter()
        .map(|n| json!({ "agent": n.agent, "note": n.note }))
        .collect();
    json!({
        "fileName": report.file_name,
        "mode": report.mode,
        "score": report.score,
        "grade": report.grade,
        "lineCount": report.line_count,
        "byteSize": report.byte_size,
        "tokenEstimate": report.token_estimate,
        "findings": findings,
        "penalties": penalties,
        "agentNotes": agent_notes,
        "notices": report.notices,
        "rulepackVersion": report.rulepack_version,
        "rulepackUpdated": report.rulepack_updated,
    })
}

/// `JSON.stringify(report, null, 2)` equivalent.
pub fn report_to_json_pretty(report: &Report) -> String {
    serde_json::to_string_pretty(&report_to_value(report)).expect("report serializes")
}

/// Compact `JSON.stringify(report)` equivalent (wasm/browser interchange).
pub fn report_to_json(report: &Report) -> String {
    serde_json::to_string(&report_to_value(report)).expect("report serializes")
}
