#![allow(dead_code)] // each integration-test binary compiles its own copy and uses a subset
//! Hermetic fixture builders. The repo fixtures used by dir-mode tests are
//! deliberately NOT in git (the Bun suite builds them at runtime too — see
//! test/cli.test.ts makeRepoFixture); each Rust test builds its own copy in a
//! temp dir so `cargo test` passes on a fresh clone. Content must stay
//! byte-identical to the Bun builders — the golden files were captured from
//! these trees.

use std::fs;
use std::path::PathBuf;

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aispekt-test-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

/// Mirror of test/cli.test.ts `makeRepoFixture()` — drift repo for dir mode.
pub fn make_repo_fixture() -> PathBuf {
    let dir = fresh_dir("repo");
    fs::create_dir_all(dir.join("src/api/handlers")).unwrap();
    fs::create_dir_all(dir.join(".github/workflows")).unwrap();
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(
        dir.join("CLAUDE.md"),
        [
            "# repo instructions",
            "@docs/conventions.md",
            "- Build: `bun run build`",
            "- Ship: `bun run deploy:prod`",
            "- Old code in `src/worker/legacy/`",
            "- CI runs `.github/workflows/ci.yml`; bundle lands in `dist/bundle.js`",
            "- Auth via `@acme/shared/auth0`; live at `demo.example.com/play`",
            "- Never commit secrets.",
        ]
        .join("\n"),
    )
    .unwrap();
    fs::write(dir.join("README.md"), "# demo\nRun `bun install`.\n").unwrap();
    fs::write(
        dir.join("package.json"),
        r#"{"name":"demo","scripts":{"build":"vite build"}}"#,
    )
    .unwrap();
    fs::write(dir.join("src/api/handlers/users.ts"), "export {};\n").unwrap();
    fs::write(dir.join(".github/workflows/ci.yml"), "name: ci\n").unwrap();
    fs::write(dir.join("dist/bundle.js"), "// built\n").unwrap();
    dir
}

/// Mirror of test/cli.test.ts ISC-23.8 setup — CLAUDE.md symlinked to AGENTS.md.
#[cfg(unix)]
pub fn make_symlink_repo() -> PathBuf {
    let dir = fresh_dir("symlink-repo");
    fs::write(
        dir.join("AGENTS.md"),
        "# corp instructions\nRun `bun test`. Never skip.\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("AGENTS.md", dir.join("CLAUDE.md")).unwrap();
    dir
}
