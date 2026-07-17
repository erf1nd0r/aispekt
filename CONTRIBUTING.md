# Contributing to aispekt

## Development

```sh
cargo test --workspace      # engine + CLI + golden parity + property tests
cargo clippy --workspace --all-targets -- -D warnings

bun install
bun test                    # TS reference engine + WASM parity
bun run build               # tsc + wasm build + vite (docs site)
```

Zero runtime dependencies in every deliverable: the binary is static, the wasm module is dependency-free, the site ships no JS framework.

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

## Updating the rulepack

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
3. Every finding in the CLI/playground links its `evidenceUrl` and shows its tier (`measured` > `official` > `community` > `heuristic`). Rules the studies contradict (like the 200-line target) are labeled `heuristic` and framed as cost, not adherence.
4. Re-verify cadence: sweep the evidence URLs (agents.md spec, Anthropic memory/best-practices docs, the arXiv studies) and refresh `lastVerified`; both doc hosts moved in the last year, so follow redirects.

Calibration is regression-tested: known-good and known-bad fixtures pin score bounds, and property tests guarantee scoring stays penalty-only (appending bloat can never raise a score).

## Releasing (maintainers)

Tag `v<version>` (matching `npm/aispekt/package.json`) and push — `.github/workflows/release.yml` cross-builds five targets, publishes the npm wrapper + platform packages, and attaches binaries to the GitHub release. Requires the `NPM_TOKEN` repo secret.
