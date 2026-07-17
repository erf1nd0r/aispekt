use crate::types::{CheckContext, CodeSpan, RepoContext};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// Regex scans are quadratic on pathological single lines; cap what they see.
/// The TS engine slices by UTF-16 code units; we mirror that exactly.
const MAX_SCAN_LINE: usize = 10_000;

static FENCE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(```|~~~)").unwrap());
static INLINE_CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

/// `s.slice(0, n)` in JS counts UTF-16 code units. If the cut would split a
/// surrogate pair we keep one unit less (a lone surrogate is unrepresentable
/// in Rust strings — documented parity divergence, unreachable for valid text).
pub fn utf16_slice(s: &str, n: usize) -> String {
    if utf16_len(s) <= n {
        return s.to_string();
    }
    let mut out = String::new();
    let mut units = 0usize;
    for ch in s.chars() {
        let w = ch.len_utf16();
        if units + w > n {
            break;
        }
        units += w;
        out.push(ch);
    }
    out
}

pub fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

/// Build the shared parsed view every check consumes. Pure and deterministic.
pub fn build_context(file_name: &str, content: &str, repo: Option<RepoContext>) -> CheckContext {
    let lines: Vec<String> = content
        .split('\n')
        .map(|l| {
            if utf16_len(l) > MAX_SCAN_LINE {
                utf16_slice(l, MAX_SCAN_LINE)
            } else {
                l.to_string()
            }
        })
        .collect();
    let mut prose = vec![true; lines.len()];
    let mut code_spans: Vec<CodeSpan> = Vec::new();

    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        if FENCE_RE.is_match(line) {
            in_fence = !in_fence;
            prose[i] = false;
            continue;
        }
        if in_fence {
            prose[i] = false;
            code_spans.push(CodeSpan {
                line: i + 1,
                text: line.clone(),
            });
            continue;
        }
        for m in INLINE_CODE_RE.captures_iter(line) {
            code_spans.push(CodeSpan {
                line: i + 1,
                text: m.get(1).map(|g| g.as_str()).unwrap_or("").to_string(),
            });
        }
    }

    CheckContext {
        file_name: file_name.to_string(),
        content: content.to_string(),
        lines,
        prose,
        code_spans,
        repo,
    }
}

pub fn excerpt_of(line: &str) -> String {
    excerpt_of_max(line, 120)
}

pub fn excerpt_of_max(line: &str, max: usize) -> String {
    let t = line.trim();
    if utf16_len(t) <= max {
        t.to_string()
    } else {
        format!("{}…", utf16_slice(t, max - 1))
    }
}

/// ~4 chars per token — the standard rough estimate for English/markdown.
/// JS `String.length` counts UTF-16 code units; mirrored here.
pub fn estimate_tokens(content: &str) -> u64 {
    (utf16_len(content) as u64).div_ceil(4)
}

static NORM_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s\-*+>]*(?:[0-9]+[.)])?\s*").unwrap());
static NORM_PUNCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[`*_~"'!?.,:;()\[\]]"#).unwrap());
static NORM_WS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

pub fn normalize_line(line: &str) -> String {
    let s = NORM_PREFIX_RE.replace(line, "");
    let s = s.to_lowercase();
    let s = NORM_PUNCT_RE.replace_all(&s, "");
    let s = NORM_WS_RE.replace_all(&s, " ");
    s.trim().to_string()
}

pub fn word_set(s: &str) -> HashSet<String> {
    s.split(' ')
        .filter(|w| utf16_len(w) > 2)
        .map(|w| w.to_string())
        .collect()
}

pub fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.iter().filter(|w| b.contains(*w)).count();
    inter as f64 / (a.len() + b.len() - inter) as f64
}

/// Normalize a repo-relative path reference for lookup.
pub fn normalize_path(p: &str) -> String {
    let p = p.strip_prefix("./").unwrap_or(p);
    p.trim_end_matches('/').to_string()
}

/// Strip shared leading directories until one of root_markers sits at root.
/// Browser folder drops include the dropped folder's own name as the first
/// segment of every path; a repo root must not carry that prefix.
pub fn strip_common_root(paths: &[String], root_markers: &[&str]) -> (Vec<String>, String) {
    let mut current: Vec<String> = paths.to_vec();
    let mut prefix = String::new();
    while !current.is_empty()
        && !root_markers.iter().any(|m| current.iter().any(|p| p == m))
        && current.iter().all(|p| p.contains('/'))
    {
        let firsts: HashSet<&str> = current
            .iter()
            .map(|p| p.split('/').next().unwrap_or(""))
            .collect();
        if firsts.len() != 1 {
            break;
        }
        let seg = firsts.into_iter().next().unwrap_or("");
        if seg.is_empty() {
            break;
        }
        prefix.push_str(seg);
        prefix.push('/');
        current = current
            .iter()
            .map(|p| p.split_once('/').map(|(_, rest)| rest).unwrap_or("").to_string())
            .collect();
    }
    (current, prefix)
}
