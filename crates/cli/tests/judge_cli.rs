//! CLI surface tests for the judge-brief protocol and skill subcommands —
//! ISC-93/95/98: emit/merge exit codes, the full emit→answer→merge loop
//! against the shipped binary, and skill install semantics.

use serde_json::{json, Value};
use std::fs;
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

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aispekt-judge-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn emit_writes_valid_brief() {
    let dir = tmp_dir("emit");
    let out = dir.join("brief.json");
    let st = bin()
        .args(["judge", "emit", &fixture("test/fixtures/bad.md"), "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&st.stderr));
    let brief: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(brief["protocol"], "aispekt-judge/1");
    assert!(!brief["tasks"].as_array().unwrap().is_empty());
}

#[test]
fn full_loop_emit_answer_merge() {
    let dir = tmp_dir("loop");
    let brief_path = dir.join("brief.json");
    let st = bin()
        .args(["judge", "emit", &fixture("test/fixtures/good.md"), "--out"])
        .arg(&brief_path)
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(0));
    let brief: Value = serde_json::from_str(&fs::read_to_string(&brief_path).unwrap()).unwrap();

    let answers = json!({
        "protocol": "aispekt-judge/1",
        "contentHash": brief["target"]["contentHash"],
        "judge": { "agent": "cli-test", "model": "none" },
        "answers": brief["tasks"].as_array().unwrap().iter()
            .map(|t| json!({ "taskId": t["id"], "findings": [] }))
            .collect::<Vec<_>>(),
    });
    let answers_path = dir.join("answers.json");
    fs::write(&answers_path, serde_json::to_string_pretty(&answers).unwrap()).unwrap();

    let st = bin().arg("judge").arg("merge").arg(&brief_path).arg(&answers_path).output().unwrap();
    assert_eq!(st.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&st.stderr));
    let stdout = String::from_utf8_lossy(&st.stdout);
    assert!(stdout.contains("unchanged; semantic verdicts never affect it"), "stdout: {stdout}");
    assert!(stdout.contains("judge: cli-test (none)"));

    // --json emits the merged report with the deterministic block intact
    let st = bin()
        .args(["judge", "merge", "--json"])
        .arg(&brief_path)
        .arg(&answers_path)
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(0));
    let merged: Value = serde_json::from_str(&String::from_utf8_lossy(&st.stdout)).unwrap();
    assert_eq!(merged["deterministic"], brief["deterministic"]);
}

#[test]
fn merge_hash_mismatch_exits_two() {
    let dir = tmp_dir("mismatch");
    let brief_path = dir.join("brief.json");
    bin()
        .args(["judge", "emit", &fixture("test/fixtures/good.md"), "--out"])
        .arg(&brief_path)
        .output()
        .unwrap();
    let answers = json!({
        "protocol": "aispekt-judge/1",
        "contentHash": "fnv1a64:dead000000000000",
        "judge": { "agent": "t", "model": "t" },
        "answers": [],
    });
    let answers_path = dir.join("answers.json");
    fs::write(&answers_path, answers.to_string()).unwrap();
    let st = bin().arg("judge").arg("merge").arg(&brief_path).arg(&answers_path).output().unwrap();
    assert_eq!(st.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&st.stderr).contains("content hash mismatch"));
}

#[test]
fn judge_without_subcommand_exits_two() {
    let st = bin().arg("judge").output().unwrap();
    assert_eq!(st.status.code(), Some(2));
}

#[test]
fn skill_print_emits_the_skill() {
    let st = bin().args(["skill", "print"]).output().unwrap();
    assert_eq!(st.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&st.stdout);
    assert!(stdout.starts_with("---\nname: aispekt-judge"), "frontmatter first");
    assert!(stdout.contains("aispekt judge emit"));
}

#[test]
fn skill_install_is_idempotent_and_refuses_to_clobber() {
    let dir = tmp_dir("skill");
    let root = dir.join("skills");
    let run = |extra: &[&str]| {
        let mut c = bin();
        c.args(["skill", "install", "--dir"]).arg(&root).args(extra);
        c.output().unwrap()
    };
    // fresh install
    let st = run(&[]);
    assert_eq!(st.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&st.stderr));
    let dest = root.join("aispekt-judge/SKILL.md");
    let installed = fs::read_to_string(&dest).unwrap();
    let repo_copy = fs::read_to_string(fixture("skills/aispekt-judge/SKILL.md")).unwrap();
    assert_eq!(installed, repo_copy, "installed skill differs from repo copy");
    // idempotent re-install
    let st = run(&[]);
    assert_eq!(st.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&st.stderr).contains("up to date"));
    // tampered file: refuse without --force, overwrite with it
    fs::write(&dest, "tampered").unwrap();
    let st = run(&[]);
    assert_eq!(st.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&st.stderr).contains("--force"));
    let st = run(&["--force"]);
    assert_eq!(st.status.code(), Some(0));
    assert_eq!(fs::read_to_string(&dest).unwrap(), repo_copy);
}
