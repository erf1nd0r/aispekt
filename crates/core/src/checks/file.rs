use crate::parse::{excerpt_of, jaccard, normalize_line, utf16_slice, word_set};
use crate::types::{CheckContext, RawFinding};
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

// JS \b and \w are ASCII; the regex crate's are Unicode. Every translated
// pattern therefore uses (?-u:\b) and explicit ASCII classes.

static LINT_LEAKAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:[0-9]+[- ]spaces?(?-u:\b)|spaces? (?:for|per) indent|indent(?:ation)? (?:with|using|of|is) |tabs?,? not spaces|spaces?,? not tabs|single quotes|double quotes|semicolons?(?-u:\b)|trailing commas?|max(?:imum)? line (?:length|width)|line length (?:of|under|max)|alphabetical(?:ly)? (?:order|sorted)|import order|sort(?:ed)? imports)").unwrap()
});

const VAGUE_PHRASES: [&str; 21] = [
    "write clean code",
    "clean, professional code",
    "clean and professional",
    "best practices",
    "be careful",
    "use common sense",
    "use good judgment",
    "handle errors appropriately",
    "handle errors properly",
    "handle errors gracefully",
    "as appropriate",
    "when appropriate",
    "as needed",
    "high quality code",
    "high-quality code",
    "meaningful names",
    "self-documenting",
    "keep it simple",
    "professional code",
    "robust code",
    "well-tested code",
];

static COMMAND_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s$>])(?:bun|bunx|npm|npx|yarn|pnpm|make|cargo|pytest|uv|pip3?|docker|git|wrangler|tsc|vite|eslint|ruff|prettier|go|swift|python3?|gradle|mvn|rake|mix|dotnet|\./[0-9A-Za-z_.-]+)(?-u:\b)").unwrap()
});

static SECRET_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"sk-ant-[A-Za-z0-9_-]{10,}").unwrap(), "Anthropic API key"),
        (Regex::new(r"sk-[A-Za-z0-9]{20,}").unwrap(), "API key (sk-...)"),
        (Regex::new(r"ghp_[A-Za-z0-9]{20,}").unwrap(), "GitHub personal access token"),
        (Regex::new(r"github_pat_[A-Za-z0-9_]{20,}").unwrap(), "GitHub fine-grained PAT"),
        (Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(), "AWS access key id"),
        (Regex::new(r"xox[baprs]-[A-Za-z0-9-]{10,}").unwrap(), "Slack token"),
        (
            Regex::new(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----").unwrap(),
            "private key block",
        ),
        (
            Regex::new(r"eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}").unwrap(),
            "JWT",
        ),
    ]
});

static EMPHASIS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(?:IMPORTANT|YOU MUST|CRITICAL|NEVER EVER)(?-u:\b)").unwrap()
});
const EMPHASIS_LIMIT: usize = 5;

// (?<![\w/]) needs lookbehind — fancy-regex. \b after .md is rewritten as a
// lookahead because fancy-regex word boundaries are not ASCII-tunable.
static DOC_REF_RE: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(r"(@[0-9A-Za-z_./-]+\.md(?![0-9A-Za-z_])|\[[^\]]*\]\([^)]*\.md[^)]*\)|(?<![0-9A-Za-z_/])[0-9A-Za-z_-]+/[0-9A-Za-z_./-]*\.md(?![0-9A-Za-z_]))").unwrap()
});
static READ_IMPERATIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:read|consult|follow|load|review|before starting|IMPORTANT)(?-u:\b)")
        .unwrap()
});

fn prose_lines(ctx: &CheckContext) -> Vec<(usize, &str)> {
    ctx.lines
        .iter()
        .enumerate()
        .filter(|(i, _)| ctx.prose[*i])
        .map(|(i, l)| (i, l.as_str()))
        .collect()
}

pub fn lint_leakage(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    for (i, line) in prose_lines(ctx) {
        if line.trim().is_empty() {
            continue;
        }
        if LINT_LEAKAGE.is_match(line) {
            out.push(RawFinding::new(
                i + 1,
                excerpt_of(line),
                "Formatter-enforceable style rule stated as prose.",
            ));
        }
    }
    out
}

static TREE_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[├└│]").unwrap());
static TREE_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,8}[0-9A-Za-z_.@-]+/\s*(?:#.*)?$").unwrap());

pub fn directory_tree(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    let mut block_start: Option<usize> = None;
    let mut run = 0usize;
    for (i, line) in ctx.lines.iter().enumerate() {
        let treeish = TREE_CHARS.is_match(line) || TREE_DIR.is_match(line);
        if treeish {
            if run == 0 {
                block_start = Some(i);
            }
            run += 1;
        } else {
            if run >= 3 {
                let bs = block_start.unwrap_or(0);
                out.push(RawFinding::new(
                    bs + 1,
                    excerpt_of(ctx.lines.get(bs).map(|s| s.as_str()).unwrap_or("")),
                    format!("Directory-tree block of {run} lines."),
                ));
            }
            run = 0;
        }
    }
    if run >= 3 {
        if let Some(bs) = block_start {
            out.push(RawFinding::new(
                bs + 1,
                excerpt_of(ctx.lines.get(bs).map(|s| s.as_str()).unwrap_or("")),
                format!("Directory-tree block of {run} lines."),
            ));
        }
    }
    out
}

pub fn vague_rules(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    for (i, line) in prose_lines(ctx) {
        let lower = line.to_lowercase();
        if line.contains('`') {
            continue; // concrete artifact present
        }
        if let Some(hit) = VAGUE_PHRASES.iter().find(|p| lower.contains(*p)) {
            out.push(RawFinding::new(
                i + 1,
                excerpt_of(line),
                format!("Vague phrase \"{hit}\" — nothing testable for the agent."),
            ));
        }
    }
    out
}

pub fn no_commands(ctx: &CheckContext) -> Vec<RawFinding> {
    for span in &ctx.code_spans {
        if COMMAND_TOKEN.is_match(&span.text) {
            return vec![];
        }
    }
    vec![RawFinding::new(
        1,
        excerpt_of(ctx.lines.first().map(|s| s.as_str()).unwrap_or("")),
        "No runnable command found anywhere in the file.",
    )]
}

static SECRET_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)secret").unwrap());
static HARD_RULE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)(?:never|always|must)(?-u:\b)").unwrap());
static ENFORCEABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:commit|push|force[- ]push|merge|deploy|format|lint|console\.log|debugger|\.env|prettier|eslint|typecheck|any(?-u:\b))").unwrap()
});

pub fn enforcement_prose(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    for (i, line) in prose_lines(ctx) {
        if SECRET_WORD_RE.is_match(line) {
            continue; // "never commit secrets" is the canonical good boundary
        }
        if !HARD_RULE_RE.is_match(line) {
            continue;
        }
        if ENFORCEABLE_RE.is_match(line) {
            out.push(RawFinding::new(
                i + 1,
                excerpt_of(line),
                "Zero-exception rule expressed as advisory prose.",
            ));
        }
    }
    out
}

pub fn secret_leak(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    for (i, line) in ctx.lines.iter().enumerate() {
        for (re, label) in SECRET_PATTERNS.iter() {
            if re.is_match(line) {
                let redacted = re
                    .replace(line, |caps: &regex::Captures| {
                        format!("{}…[redacted]", utf16_slice(&caps[0], 8))
                    })
                    .to_string();
                out.push(RawFinding::new(
                    i + 1,
                    excerpt_of(&redacted),
                    format!("Token-shaped string: {label}."),
                ));
                break;
            }
        }
    }
    out
}

pub fn emphasis_overuse(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut count = 0usize;
    let mut overflow_line = 0usize;
    for (i, line) in ctx.lines.iter().enumerate() {
        if !ctx.prose[i] {
            continue;
        }
        let matches = EMPHASIS_RE.find_iter(line).count();
        if matches > 0 {
            count += matches;
            if count > EMPHASIS_LIMIT && overflow_line == 0 {
                overflow_line = i + 1;
            }
        }
    }
    if count > EMPHASIS_LIMIT {
        return vec![RawFinding::new(
            overflow_line,
            excerpt_of(
                ctx.lines
                    .get(overflow_line - 1)
                    .map(|s| s.as_str())
                    .unwrap_or(""),
            ),
            format!(
                "{count} emphasis markers (IMPORTANT/YOU MUST/CRITICAL) — past ~{EMPHASIS_LIMIT} they cancel out."
            ),
        )];
    }
    vec![]
}

pub fn pointer_without_instruction(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut ref_lines: Vec<usize> = Vec::new();
    for (i, line) in prose_lines(ctx) {
        if DOC_REF_RE.is_match(line).unwrap_or(false) {
            ref_lines.push(i);
        }
    }
    // cluster: >=3 doc-ref lines within an 8-line window
    let mut a = 0usize;
    while a + 2 < ref_lines.len() {
        let start = ref_lines[a];
        let third = ref_lines[a + 2];
        if third - start <= 8 {
            let lo = start.saturating_sub(3);
            let hi = (third + 1).min(ctx.lines.len().saturating_sub(1));
            let mut has_imperative = false;
            for i in lo..=hi {
                if READ_IMPERATIVE_RE.is_match(ctx.lines.get(i).map(|s| s.as_str()).unwrap_or(""))
                {
                    has_imperative = true;
                    break;
                }
            }
            if !has_imperative {
                return vec![RawFinding::new(
                    start + 1,
                    excerpt_of(ctx.lines.get(start).map(|s| s.as_str()).unwrap_or("")),
                    "Cluster of doc links with no instruction to read them.",
                )];
            }
            return vec![];
        }
        a += 1;
    }
    vec![]
}

pub fn size_cost(ctx: &CheckContext) -> Vec<RawFinding> {
    if ctx.lines.len() > 200 {
        return vec![RawFinding::new(
            201,
            excerpt_of(ctx.lines.get(200).map(|s| s.as_str()).unwrap_or("")),
            format!(
                "{} lines — Anthropic's target is under 200. (Cost argument, not adherence — see the cited caveat.)",
                ctx.lines.len()
            ),
        )];
    }
    vec![]
}

pub fn duplicate_rules(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for (i, line) in prose_lines(ctx) {
        let norm = normalize_line(line);
        if crate::parse::utf16_len(&norm) < 20 {
            continue;
        }
        if let Some(first) = seen.get(&norm) {
            out.push(RawFinding::new(
                i + 1,
                excerpt_of(line),
                format!("Duplicates line {}.", first + 1),
            ));
        } else {
            seen.insert(norm, i);
        }
    }
    out
}

static TABS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)tabs?(?-u:\b)").unwrap());
static INDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)indent").unwrap());
static SPACES_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:(?-u:\b)[0-9]+[- ]spaces?(?-u:\b)|spaces? for indent)").unwrap()
});
// (?= …) lookahead — fancy-regex; \b rewritten as ASCII lookarounds.
static MANAGER_RE: LazyLock<FancyRegex> = LazyLock::new(|| {
    FancyRegex::new(r"(?<![0-9A-Za-z_])(npm|yarn|pnpm|bun)(?![0-9A-Za-z_])(?= (?:run|install|test|add|create|x)(?![0-9A-Za-z_])| )").unwrap()
});

pub fn contradiction_lite(ctx: &CheckContext) -> Vec<RawFinding> {
    let mut out = Vec::new();
    let prose = prose_lines(ctx);
    let tab_line = prose
        .iter()
        .find(|(_, line)| TABS_RE.is_match(line) && INDENT_RE.is_match(line));
    let space_line = prose
        .iter()
        .find(|(_, line)| SPACES_RE.is_match(line) && INDENT_RE.is_match(line));
    if let (Some((ti, _)), Some((si, _))) = (tab_line, space_line) {
        if ti != si {
            let max_i = (*ti).max(*si);
            out.push(RawFinding::new(
                max_i + 1,
                excerpt_of(ctx.lines.get(max_i).map(|s| s.as_str()).unwrap_or("")),
                format!(
                    "Tabs (line {}) vs spaces (line {}) for indentation.",
                    ti + 1,
                    si + 1
                ),
            ));
        }
    }
    let mut managers: Vec<(String, usize)> = Vec::new();
    for span in &ctx.code_spans {
        if let Ok(Some(caps)) = MANAGER_RE.captures(&span.text) {
            let name = caps.get(1).map(|g| g.as_str()).unwrap_or("").to_string();
            if !name.is_empty() && !managers.iter().any(|(n, _)| *n == name) {
                managers.push((name, span.line));
            }
        }
    }
    if managers.len() >= 2 {
        let mut names: Vec<&str> = managers.iter().map(|(n, _)| n.as_str()).collect();
        names.sort_unstable();
        let line = managers.iter().map(|(_, l)| *l).max().unwrap_or(1);
        out.push(RawFinding::new(
            line,
            excerpt_of(ctx.lines.get(line - 1).map(|s| s.as_str()).unwrap_or("")),
            format!("Mixed package managers in commands: {}.", names.join(", ")),
        ));
    }
    out
}

static BOUNDARY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:never|don'?t|do not|must not|ask (?:first|before))(?-u:\b)")
        .unwrap()
});

pub fn boundaries_missing(ctx: &CheckContext) -> Vec<RawFinding> {
    for (_, line) in prose_lines(ctx) {
        if BOUNDARY_RE.is_match(line) {
            return vec![];
        }
    }
    vec![RawFinding::new(
        1,
        excerpt_of(ctx.lines.first().map(|s| s.as_str()).unwrap_or("")),
        "No never/ask-first boundary found.",
    )]
}

pub fn codex_cap(ctx: &CheckContext) -> Vec<RawFinding> {
    let bytes = ctx.content.len();
    if bytes > 32 * 1024 {
        let kib = crate::render::to_fixed1(bytes as f64 / 1024.0);
        return vec![RawFinding::new(
            1,
            format!("{kib} KiB"),
            format!("File is {kib} KiB — Codex silently truncates at 32 KiB."),
        )];
    }
    vec![]
}

/// README-redundancy lives here for reuse but is a repo-scoped rule.
pub fn redundant_with(line: &str, readme_norms: &[(String, HashSet<String>)]) -> bool {
    let norm = normalize_line(line);
    if crate::parse::utf16_len(&norm) < 40 {
        return false;
    }
    let words = word_set(&norm);
    for (rnorm, rwords) in readme_norms {
        if *rnorm == norm {
            return true;
        }
        if words.len() >= 6 && jaccard(&words, rwords) >= 0.8 {
            return true;
        }
    }
    false
}
