use crate::checks::file::redundant_with;
use crate::parse::{excerpt_of, normalize_line, normalize_path, word_set};
use crate::types::{CheckContext, RawFinding, RepoContext};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// True when the reference is present — or cannot be proven absent. Entries
/// with a trailing "/" are unverifiable-directory markers (dirs the walker
/// deliberately skipped, e.g. dist/, node_modules/): anything under them
/// passes. Comparison is case-insensitive (macOS default fs). A reference
/// that exists as a suffix of some real path (context-relative mention like
/// `lib/route.ts` for client/src/lib/route.ts) also passes — flagging only
/// what is confidently absent.
fn path_exists(repo: &RepoContext, reference: &str) -> bool {
    let p = normalize_path(reference).to_lowercase();
    if p.is_empty() {
        return true;
    }
    for existing in &repo.paths {
        let is_dir_marker = existing.ends_with('/');
        let e = normalize_path(existing).to_lowercase();
        if e == p || e.starts_with(&format!("{p}/")) {
            return true;
        }
        if is_dir_marker && p.starts_with(&format!("{e}/")) {
            return true;
        }
        if e.ends_with(&format!("/{p}")) {
            return true;
        }
    }
    false
}

fn repo_content<'a>(repo: &'a RepoContext, name: &str) -> Option<&'a str> {
    let target = name.to_lowercase();
    for (key, value) in repo.contents.iter() {
        if normalize_path(key).to_lowercase() == target {
            return Some(value.as_str());
        }
    }
    None
}

static RUN_SCRIPT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(?:bun|npm|pnpm|yarn) run ([0-9A-Za-z_:.-]+)").unwrap()
});

pub fn dead_command(ctx: &CheckContext) -> Vec<RawFinding> {
    let Some(repo) = &ctx.repo else { return vec![] };
    let Some(pkg_raw) = repo_content(repo, "package.json") else {
        return vec![];
    };
    let scripts: HashSet<String> = match serde_json::from_str::<serde_json::Value>(pkg_raw) {
        Ok(pkg) => pkg
            .get("scripts")
            .and_then(|s| s.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    let mut reported: HashSet<String> = HashSet::new();
    for span in &ctx.code_spans {
        for caps in RUN_SCRIPT_RE.captures_iter(&span.text) {
            let script = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            if !script.is_empty() && !scripts.contains(script) && !reported.contains(script) {
                reported.insert(script.to_string());
                out.push(RawFinding::new(
                    span.line,
                    excerpt_of(&span.text),
                    format!("Script \"{script}\" is not in package.json scripts."),
                ));
            }
        }
    }
    out
}

static FILE_EXTENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:md|markdown|json|ts|js|tsx|jsx|mjs|cjs|toml|yaml|yml|txt|css|html|svg|png|jpg|lock|env|sh|py|rs|go|swift)$").unwrap()
});
static HOST_TLD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z]{2,}$").unwrap());

/// "fives.gugus.gg" is a hostname; "wrangler.toml" and ".github" are not.
fn is_hostname_like(segment: &str) -> bool {
    if !segment.contains('.') || segment.starts_with('.') {
        return false;
    }
    let last = segment.split('.').next_back().unwrap_or("");
    HOST_TLD_RE.is_match(last) && !FILE_EXTENSIONS.is_match(last)
}

static PATH_LIKE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\.{0,2}/?[0-9A-Za-z_@.-]+(?:/[0-9A-Za-z_@.-]+)+/?$").unwrap()
});
static VERSION_SEG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^v?[0-9]+(?:[.-][0-9]+)*$").unwrap());

pub fn stale_path(ctx: &CheckContext) -> Vec<RawFinding> {
    let Some(repo) = &ctx.repo else { return vec![] };
    // A truncated walk cannot prove absence — never flag from a partial list.
    if repo.paths.is_empty() || repo.truncated {
        return vec![];
    }
    let mut out = Vec::new();
    let mut reported: HashSet<String> = HashSet::new();
    for span in &ctx.code_spans {
        let token = span.text.trim();
        // path-like: contains a slash, no spaces, no URL, no glob
        if !PATH_LIKE_RE.is_match(token) || token.contains("://") {
            continue;
        }
        // npm package specifiers (@scope/pkg/subpath) are module ids, not files
        if token.starts_with('@') {
            continue;
        }
        // hostname-like first segment (fives.gugus.gg/play) is a URL, not a path
        if !token.starts_with("./")
            && is_hostname_like(normalize_path(token).split('/').next().unwrap_or(""))
        {
            continue;
        }
        let norm = normalize_path(token);
        // trailing bare version segment = identifier (git tag, package route),
        // not a file path: shopnavigation/mainnavigationpath/1.2.3
        let last_seg = norm.split('/').next_back().unwrap_or("");
        if VERSION_SEG_RE.is_match(last_seg) {
            continue;
        }
        if reported.contains(&norm) {
            continue;
        }
        if !path_exists(repo, &norm) {
            reported.insert(norm.clone());
            out.push(RawFinding::new(
                span.line,
                excerpt_of(token),
                format!("Path \"{norm}\" does not exist in the repo."),
            ));
        }
    }
    out
}

pub fn readme_redundancy(ctx: &CheckContext) -> Vec<RawFinding> {
    let Some(repo) = &ctx.repo else { return vec![] };
    let Some(readme) = repo_content(repo, "README.md") else {
        return vec![];
    };
    let readme_norms: Vec<(String, HashSet<String>)> = readme
        .split('\n')
        .map(normalize_line)
        .filter(|n| crate::parse::utf16_len(n) >= 40)
        .map(|norm| {
            let words = word_set(&norm);
            (norm, words)
        })
        .collect();
    if readme_norms.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    for i in 0..ctx.lines.len() {
        if out.len() >= 10 {
            break;
        }
        if !ctx.prose[i] {
            continue;
        }
        let line = &ctx.lines[i];
        if redundant_with(line, &readme_norms) {
            out.push(RawFinding::new(
                i + 1,
                excerpt_of(line),
                "Restates the README the agent already reads.",
            ));
        }
    }
    out
}

pub fn bridge_missing(ctx: &CheckContext) -> Vec<RawFinding> {
    let Some(repo) = &ctx.repo else { return vec![] };
    let has_agents = repo
        .paths
        .iter()
        .any(|p| normalize_path(p).to_lowercase() == "agents.md");
    if !has_agents {
        return vec![];
    }
    let has_claude = repo
        .paths
        .iter()
        .any(|p| normalize_path(p).to_lowercase() == "claude.md");
    if !has_claude {
        if repo.symlinks_hidden {
            let mut f = RawFinding::new(
                1,
                "AGENTS.md present, CLAUDE.md not visible",
                "No CLAUDE.md is visible, so Claude Code may never read this AGENTS.md. Browsers hide symlinks from folder drops, so a symlinked CLAUDE.md would not show up here.",
            );
            f.recommendation = Some(
                "If CLAUDE.md already exists as a symlink, this is a false alarm — confirm with the CLI, which follows symlinks. If there is truly no CLAUDE.md, add a one-line one containing `@AGENTS.md`."
                    .to_string(),
            );
            return vec![f];
        }
        return vec![RawFinding::new(
            1,
            "AGENTS.md present, CLAUDE.md absent",
            "AGENTS.md exists but Claude Code will never read it.",
        )];
    }
    let claude = repo_content(repo, "CLAUDE.md");
    let agents = repo_content(repo, "AGENTS.md");
    // A symlinked or copied CLAUDE.md reads back identical to AGENTS.md — bridged.
    if let (Some(c), Some(a)) = (claude, agents) {
        if c == a {
            return vec![];
        }
    }
    static AGENTS_IMPORT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"@AGENTS\.md(?-u:\b)").unwrap());
    if let Some(c) = claude {
        if !AGENTS_IMPORT_RE.is_match(c) {
            return vec![RawFinding::new(
                1,
                "CLAUDE.md lacks @AGENTS.md",
                "CLAUDE.md exists but does not import @AGENTS.md — the two files can drift.",
            )];
        }
    }
    vec![]
}

static AT_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\s)@((?:[0-9A-Za-z_.-]+/)*[0-9A-Za-z_.-]+\.(?:md|json))(?-u:\b)").unwrap()
});

pub fn import_resolution(ctx: &CheckContext) -> Vec<RawFinding> {
    let Some(repo) = &ctx.repo else { return vec![] };
    if repo.paths.is_empty() || repo.truncated {
        return vec![];
    }
    let mut out = Vec::new();
    let mut reported: HashSet<String> = HashSet::new();
    for i in 0..ctx.lines.len() {
        if !ctx.prose[i] {
            continue;
        }
        let line = &ctx.lines[i];
        for caps in AT_IMPORT_RE.captures_iter(line) {
            let reference = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            if reference.starts_with('~') {
                continue; // home-dir imports can't be verified against the repo
            }
            let norm = normalize_path(reference);
            if reported.contains(&norm) {
                continue;
            }
            if !path_exists(repo, &norm) {
                reported.insert(norm.clone());
                out.push(RawFinding::new(
                    i + 1,
                    excerpt_of(line),
                    format!("@-import \"{norm}\" does not resolve in the repo."),
                ));
            }
        }
    }
    out
}
