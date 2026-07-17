//! CLI behavior tests — ports of test/cli.test.ts: exit codes, --json, --min
//! parsing, --version, unknown arguments.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aispekt"))
}

fn fixture(rel: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn good_file_exits_zero() {
    let out = bin().arg(fixture("test/fixtures/good.md")).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("100/100 (A)"), "stdout: {stdout}");
}

#[test]
fn bad_file_exits_one_below_min() {
    let out = bin().arg(fixture("test/fixtures/bad.md")).output().unwrap();
    assert_eq!(out.status.code(), Some(1)); // score 31 < default min 60
}

#[test]
fn min_threshold_overrides() {
    let out = bin()
        .args([fixture("test/fixtures/bad.md"), "--min".into(), "10".into()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let out = bin()
        .args([fixture("test/fixtures/good.md"), "--min".into(), "101".into()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn min_requires_number() {
    for bad in ["abc", "", " "] {
        let out = bin()
            .args([fixture("test/fixtures/good.md"), "--min".into(), bad.into()])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "--min {bad:?}");
    }
    let out = bin()
        .args([fixture("test/fixtures/good.md"), "--min".into()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "--min with no value");
}

#[test]
fn json_output_parses() {
    let out = bin()
        .args([fixture("test/fixtures/bad.md"), "--json".into()])
        .output()
        .unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("valid JSON on stdout");
    assert_eq!(report["fileName"], "bad.md");
    assert_eq!(report["mode"], "file");
    assert!(report["findings"].as_array().unwrap().len() > 10);
}

#[test]
fn dir_mode_runs_repo_checks() {
    let out = bin()
        .args([fixture("test/fixtures/repo"), "--json".into()])
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["mode"], "repo");
    let rules: Vec<&str> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["ruleId"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"dead-command"), "rules: {rules:?}");
    assert!(rules.contains(&"stale-path"), "rules: {rules:?}");
}

#[test]
fn version_flag() {
    let out = bin().arg("--version").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v = String::from_utf8_lossy(&out.stdout);
    assert_eq!(v.trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn unknown_argument_exits_two() {
    let out = bin()
        .args([fixture("test/fixtures/good.md"), "--nope".into()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn missing_target_exits_two() {
    let out = bin().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("usage:"), "stderr: {err}");
}

#[test]
fn nonexistent_target_exits_two() {
    let out = bin().arg("/definitely/not/a/real/path").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn dir_without_instruction_file_exits_two() {
    let dir = std::env::temp_dir().join("aispekt-empty-dir-test");
    std::fs::create_dir_all(&dir).unwrap();
    let out = bin().arg(&dir).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no CLAUDE.md or AGENTS.md"), "stderr: {err}");
}
