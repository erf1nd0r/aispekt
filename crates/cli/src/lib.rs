//! aispekt CLI — same engine and rulepack as the playground, real filesystem.
//!
//!   aispekt <file-or-dir> [--json] [--min <score>]
//!
//! Exits 0 when score >= min (default 60), 1 below, 2 on usage/IO errors.

use aispekt_core::types::{AnalysisInput, Report, Severity};
use aispekt_core::{ContentMap, RepoContext};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const SKIP_DIRS: [&str; 8] = [
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    ".venv",
    "vendor",
    "coverage",
];
const MAX_FILES: usize = 200_000;
const INSTRUCTION_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];

fn rel_str(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Breadth-first: root files always enumerate before deep subtrees.
/// Returns (sorted paths, truncated).
pub fn walk(root: &Path) -> (Vec<String>, bool) {
    let mut out: Vec<String> = Vec::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::from([root.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let abs = entry.path();
            let rel = rel_str(root, &abs);
            let Ok(ft) = entry.file_type() else { continue };
            let is_dir = ft.is_dir();
            let mut is_file = ft.is_file();
            if ft.is_symlink() {
                // Follow file symlinks (a symlinked CLAUDE.md is a real bridge);
                // mark dir symlinks unverifiable instead of recursing (cycle-safe).
                match fs::metadata(&abs) {
                    Ok(st) if st.is_file() => is_file = true,
                    Ok(st) if st.is_dir() => {
                        out.push(format!("{rel}/"));
                        continue;
                    }
                    _ => continue, // broken symlink
                }
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_dir {
                if name == ".git" {
                    continue;
                }
                if SKIP_DIRS.contains(&name.as_str()) {
                    out.push(format!("{rel}/")); // unverifiable-directory marker
                    continue;
                }
                queue.push_back(abs);
            } else if is_file {
                if out.len() >= MAX_FILES {
                    out.sort_unstable();
                    return (out, true);
                }
                out.push(rel);
            }
        }
    }
    out.sort_unstable();
    (out, false)
}

fn read_lossy(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn build_input(target: &Path) -> Result<AnalysisInput, String> {
    let st = fs::metadata(target).map_err(|e| format!("{}: {e}", target.display()))?;
    if st.is_file() {
        let file_name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(AnalysisInput {
            file_name,
            content: read_lossy(target)?,
            repo: None,
        });
    }
    let (paths, truncated) = walk(target);
    let instruction = INSTRUCTION_FILES
        .iter()
        .find(|f| paths.iter().any(|p| p == *f))
        .ok_or_else(|| format!("no CLAUDE.md or AGENTS.md at the root of {}", target.display()))?;
    let mut contents = ContentMap::default();
    for name in ["CLAUDE.md", "AGENTS.md", "package.json", "README.md"] {
        if paths.iter().any(|p| p == name) {
            contents.insert(name.to_string(), read_lossy(&target.join(name))?);
        }
    }
    let content = contents
        .iter()
        .find(|(k, _)| k.as_str() == *instruction)
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    Ok(AnalysisInput {
        file_name: instruction.to_string(),
        content,
        repo: Some(RepoContext {
            paths,
            contents,
            truncated,
            symlinks_hidden: false,
        }),
    })
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

fn severity_color(s: Severity) -> &'static str {
    match s {
        Severity::Error => "\x1b[31m",
        Severity::Warn => "\x1b[33m",
        Severity::Info => "\x1b[36m",
    }
}

pub fn print_human(report: &Report) -> String {
    let mut o = String::new();
    let grade_color = match report.grade {
        "A" | "B" => "\x1b[32m",
        "C" => "\x1b[33m",
        _ => "\x1b[31m",
    };
    let _ = writeln!(
        o,
        "\n{BOLD}aispekt{RESET} — {} ({} mode)",
        report.file_name, report.mode
    );
    let _ = writeln!(
        o,
        "{BOLD}{grade_color}{}/100 ({}){RESET}  ·  {} lines · ~{} tokens loaded every session · rulepack v{}\n",
        report.score, report.grade, report.line_count, report.token_estimate, report.rulepack_version
    );
    for n in &report.notices {
        let _ = writeln!(o, "  ⚠️  {n}\n");
    }
    if report.findings.is_empty() {
        let _ = writeln!(o, "  No findings. Lean file.\n");
    }
    for f in &report.findings {
        let _ = writeln!(
            o,
            "  {}{:<5}{RESET} L{:>4}  {BOLD}{}{RESET} [{}]",
            severity_color(f.severity),
            f.severity.as_str().to_uppercase(),
            f.line,
            f.rule_name,
            f.evidence_tier.as_str()
        );
        let _ = writeln!(o, "        {}", f.message);
        if !f.excerpt.is_empty() {
            let _ = writeln!(o, "        > {}", f.excerpt);
        }
        let _ = writeln!(o, "        fix: {}", f.recommendation);
        let _ = writeln!(o, "        evidence: {}\n", f.evidence_url);
    }
    for n in &report.agent_notes {
        let _ = writeln!(o, "  {BOLD}{}:{RESET} {}", n.agent, n.note);
    }
    o
}
