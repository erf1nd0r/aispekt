//! Golden parity: the Rust engine's `--json` output must be byte-identical to
//! the frozen TypeScript oracle (test/golden/expected/*.json, generated with
//! `bun src/cli.ts <input> --json`). Regenerate the oracle only deliberately.
//!
//! The suite is driven by the expected/ directory listing, so every golden on
//! disk is automatically covered — adding a corpus+expected pair needs no
//! registration here, and forgetting the expected file (or the input) fails
//! loudly instead of silently shrinking coverage.

mod common;

use aispekt::build_input;
use aispekt_core::{analyze, report_to_json_pretty};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Resolve a golden name to its input path. Repo-mode goldens map either to a
/// committed corpus repo or to a hermetic builder (mirroring the Bun suite's
/// runtime fixtures, which are deliberately not in git).
fn input_for(name: &str) -> Option<PathBuf> {
    let root = repo_root();
    match name {
        "repo-repo" => Some(common::make_repo_fixture()),
        "repo-symlink-repo" => {
            #[cfg(unix)]
            {
                Some(common::make_symlink_repo())
            }
            #[cfg(not(unix))]
            {
                None // symlink creation needs privileges on Windows
            }
        }
        _ => {
            if let Some(repo) = name.strip_prefix("repo-") {
                // goldens are repo-<x>.json; corpus dirs are <x>-repo/
                let direct = root.join("test/golden/corpus").join(repo);
                if direct.exists() {
                    return Some(direct);
                }
                return Some(root.join("test/golden/corpus").join(format!("{repo}-repo")));
            }
            let corpus = root.join("test/golden/corpus").join(format!("{name}.md"));
            if corpus.exists() {
                return Some(corpus);
            }
            Some(root.join("test/fixtures").join(format!("{name}.md")))
        }
    }
}

fn assert_golden_at(input_path: &Path, golden_path: &Path) {
    let name = golden_path.file_name().unwrap().to_string_lossy();
    let input = build_input(input_path)
        .unwrap_or_else(|e| panic!("build_input for golden {name}: {e}"));
    let report = analyze(&input);
    let actual = format!("{}\n", report_to_json_pretty(&report));
    let expected = std::fs::read_to_string(golden_path)
        .unwrap_or_else(|e| panic!("read golden {name}: {e}"));
    if actual != expected {
        // Byte-level context makes regex/rounding divergences findable fast.
        let mismatch = actual
            .lines()
            .zip(expected.lines())
            .enumerate()
            .find(|(_, (a, e))| a != e);
        if let Some((idx, (a, e))) = mismatch {
            panic!(
                "golden mismatch for {name} at line {}:\n  rust: {a}\n  ts:   {e}",
                idx + 1
            );
        }
        panic!(
            "golden mismatch for {name}: line counts differ (rust {}, ts {})",
            actual.lines().count(),
            expected.lines().count()
        );
    }
}

#[test]
fn every_golden_on_disk_matches() {
    let expected_dir = repo_root().join("test/golden/expected");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&expected_dir)
        .expect("list test/golden/expected")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();
    assert!(
        entries.len() >= 17,
        "golden corpus shrank: only {} expected files found",
        entries.len()
    );
    for golden_path in entries {
        let name = golden_path.file_stem().unwrap().to_string_lossy().to_string();
        match input_for(&name) {
            Some(input) => assert_golden_at(&input, &golden_path),
            None => eprintln!("skipping {name}: unsupported on this platform"),
        }
    }
}
