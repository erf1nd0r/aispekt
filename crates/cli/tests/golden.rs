//! Golden parity: the Rust engine's `--json` output must be byte-identical to
//! the frozen TypeScript oracle (test/golden/expected/*.json, generated with
//! `bun src/cli.ts <input> --json`). Regenerate the oracle only deliberately.

use aispekt::build_input;
use aispekt_core::{analyze, report_to_json_pretty};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn assert_golden(input_rel: &str, golden_name: &str) {
    let root = repo_root();
    let input = build_input(&root.join(input_rel)).expect("build_input");
    let report = analyze(&input);
    let actual = format!("{}\n", report_to_json_pretty(&report));
    let golden_path = root.join("test/golden/expected").join(golden_name);
    let expected = std::fs::read_to_string(&golden_path).expect("golden file");
    if actual != expected {
        // Byte-level context makes regex/rounding divergences findable fast.
        let mismatch = actual
            .lines()
            .zip(expected.lines())
            .enumerate()
            .find(|(_, (a, e))| a != e);
        if let Some((idx, (a, e))) = mismatch {
            panic!(
                "golden mismatch for {golden_name} at line {}:\n  rust: {a}\n  ts:   {e}",
                idx + 1
            );
        }
        panic!(
            "golden mismatch for {golden_name}: line counts differ (rust {}, ts {})",
            actual.lines().count(),
            expected.lines().count()
        );
    }
}

macro_rules! golden_file {
    ($name:ident, $input:expr, $golden:expr) => {
        #[test]
        fn $name() {
            assert_golden($input, $golden);
        }
    };
}

golden_file!(good, "test/fixtures/good.md", "good.json");
golden_file!(bad, "test/fixtures/bad.md", "bad.json");
golden_file!(contradictions, "test/golden/corpus/contradictions.md", "contradictions.json");
golden_file!(emphasis, "test/golden/corpus/emphasis.md", "emphasis.json");
golden_file!(empty, "test/golden/corpus/empty.md", "empty.json");
golden_file!(hugefile, "test/golden/corpus/hugefile.md", "hugefile.json");
golden_file!(longfile, "test/golden/corpus/longfile.md", "longfile.json");
golden_file!(minimal, "test/golden/corpus/minimal.md", "minimal.json");
golden_file!(secrets, "test/golden/corpus/secrets.md", "secrets.json");
golden_file!(unicode, "test/golden/corpus/unicode.md", "unicode.json");
golden_file!(vague, "test/golden/corpus/vague.md", "vague.json");
golden_file!(ignores, "test/golden/corpus/ignores.md", "ignores.json");
golden_file!(repo_fixture, "test/fixtures/repo", "repo-repo.json");
golden_file!(repo_symlink, "test/fixtures/symlink-repo", "repo-symlink-repo.json");
golden_file!(repo_bridge, "test/golden/corpus/bridge-repo", "repo-bridge.json");
golden_file!(repo_drift, "test/golden/corpus/drift-repo", "repo-drift.json");
