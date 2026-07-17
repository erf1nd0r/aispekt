# 🔍 aispekt

**Is your AGENTS.md / CLAUDE.md helping your coding agent — or silently taxing it?**

aispekt is a fast CLI (single native Rust binary, ~1.4 MB, ~20 ms per repo) that scores your agent instruction file and gives evidence-cited recommendations: what to cut, what to fix, and the study or official doc behind every finding.

```
bunx aispekt .          # or: npx aispekt .
```

Docs + browser playground: **https://aispekt.erfindor.com**

## Install

```
# npm / bun (prebuilt binary via platform packages)
npm install -g aispekt

# cargo (build from source)
cargo install --locked --git https://github.com/erf1nd0r/aispekt aispekt
```

Prebuilt binaries for macOS/Linux/Windows are attached to [GitHub releases](https://github.com/erf1nd0r/aispekt/releases).

## Usage

```
aispekt <file-or-dir> [--json] [--min <score>]
```

Point it at a single file, or at a repo directory to unlock **repo-aware checks**: dead commands vs `package.json`, stale paths, README redundancy, missing CLAUDE.md ↔ AGENTS.md bridge, unresolved `@`-imports.

Exit codes: 0 when score ≥ min (default 60), 1 below, 2 on usage/IO errors — drop it into CI or a pre-commit hook:

```
bunx aispekt . --min 75
```

## Why subtractive?

The 2026 evidence flipped the old "write a thorough agents file" advice:

- Auto-generated/bloated context files **lowered** agent success and raised cost >20% (ETH Zurich, [arXiv:2602.11988](https://arxiv.org/abs/2602.11988)). Hand-written files helped (+4%) only when carrying non-redundant, repo-specific facts.
- File *structure* (length, position, architecture) showed **no measurable effect** on compliance in a 1,650-session factorial study ([arXiv:2605.10039](https://arxiv.org/abs/2605.10039)) — content correctness is what matters.
- The documented failure modes are subtractive: Lint Leakage (62% of files), Context Bloat (42%), Skill Leakage (35%) ([arXiv:2606.15828](https://arxiv.org/abs/2606.15828)).

So aispekt scores by **penalty only** — it never rewards padding, and a lean 30-line file with exact commands and real gotchas scores 100.

## How it stays current (the rulepack)

The ecosystem moves fast; hardcoded rules rot. aispekt separates the **engine** (check implementations, Rust) from the **rulepack** (`rules/rulepack.json`) — the versioned data that carries each rule's weight, severity, confidence tier, and citation:

```json
{
  "id": "directory-tree",
  "weight": 5,
  "evidenceTier": "measured",
  "evidenceUrl": "https://arxiv.org/abs/2602.11988",
  "lastVerified": "2026-07-14"
}
```

**Updating the rubric is a data edit, not a code change:**

1. Re-weight / retire / re-cite a rule → edit `rules/rulepack.json`, bump `version` (semver: patch = wording, minor = new rule, major = rule changed/removed), set `lastVerified`.
2. New rule → add the JSON entry *and* a check implementation; the engine **fails at test time** if pack and implementations disagree in either direction, so drift is impossible to ship.
3. Every finding in the CLI/playground links its `evidenceUrl` and shows its tier (`measured` > `official` > `community` > `heuristic`). Rules the studies contradict (like the 200-line target) are labeled `heuristic` and framed as cost, not adherence — honesty is a feature.
4. Re-verify cadence: sweep the evidence URLs (agents.md spec, Anthropic memory/best-practices docs, the arXiv studies) and refresh `lastVerified`; both doc hosts moved in the last year, so follow redirects.

Calibration is regression-tested: known-good and known-bad fixtures pin score bounds, and property tests guarantee scoring stays penalty-only (appending bloat can never raise a score).

## Suppressing a finding

Heuristics have edges. Add an `aispekt-ignore` marker (the legacy `prune-ignore` also works) on the flagged line or the line above to suppress it:

```markdown
<!-- aispekt-ignore -->
- Release tags look like `shopnavigation/mainnavigationpath/1.2.3`
```

(Version-suffixed identifiers like git tags are already skipped automatically.)

## Architecture

```
crates/core   the engine: 18 checks + scoring, pure and deterministic (Rust)
crates/cli    the product: `aispekt` binary (filesystem walk, human/JSON output)
crates/wasm   the same engine compiled to WASM for the docs-site playground
rules/        rulepack.json — versioned rule data, embedded at compile time
src/, test/   TypeScript reference engine + web docs/playground frontend
npm/          npm wrapper package (platform-binary distribution)
```

The Rust engine is a semantics-preserving port of the original TypeScript engine, held to **byte-identical JSON output** by a golden-corpus parity suite (`crates/cli/tests/golden.rs` against `test/golden/`) — regenerate goldens with `bun src/cli.ts <input> --json` only when a rule deliberately changes. The WASM build is additionally cross-checked against the TS engine in `test/wasm.test.ts`.

## Roadmap (v2 candidates)

- LLM-judge layer (bring-your-own-key) for semantic checks: "is this rule dead weight for a strong model?"
- Behavioral A/B tier: run an agent on sample tasks with/without the file — the only ground-truth measurement.
- Rulepack fetched from a registry at runtime (Semgrep-style) instead of bundled.

## Development

```
cargo test --workspace      # engine + CLI + golden parity + property tests
cargo clippy --workspace --all-targets -- -D warnings

bun install
bun test                    # TS reference engine + WASM parity
bun run build               # tsc + wasm build + vite (docs site)
```

Zero runtime dependencies in every deliverable: the binary is static, the wasm module is dependency-free, the site ships no JS framework.

## Releasing

Tag `v<version>` (matching `npm/aispekt/package.json`) and push — `.github/workflows/release.yml` cross-builds five targets, publishes the npm wrapper + platform packages, and attaches binaries to the GitHub release. Requires the `NPM_TOKEN` repo secret.
