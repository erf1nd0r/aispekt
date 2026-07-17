# 🔍 aispekt

[![npm](https://img.shields.io/npm/v/aispekt)](https://www.npmjs.com/package/aispekt)
[![CI](https://github.com/erf1nd0r/aispekt/actions/workflows/ci.yml/badge.svg)](https://github.com/erf1nd0r/aispekt/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Is your AGENTS.md / CLAUDE.md helping your coding agent — or silently taxing it?**

aispekt scores your agent instruction file against published evidence and tells you what to cut, what to fix, and *why* — with the study or official doc behind every finding. It's a single native binary (Rust, ~1.4 MB, no runtime); a full repo analysis takes about 20 ms.

```sh
bunx aispekt .          # or: npx aispekt .
```

Prefer not to install anything? The **[browser playground](https://aispekt.erfindor.com)** runs the same engine compiled to WebAssembly, entirely client-side — nothing is uploaded, ever.

## Install

```sh
# npm / bun (prebuilt binary via platform packages)
npm install -g aispekt

# cargo (build from source)
cargo install --locked --git https://github.com/erf1nd0r/aispekt aispekt
```

Prebuilt binaries for macOS, Linux, and Windows are attached to [GitHub releases](https://github.com/erf1nd0r/aispekt/releases).

## Usage

```
aispekt <file-or-dir> [--json] [--min <score>]
```

Point it at a single `CLAUDE.md`/`AGENTS.md` — or at a repo directory to unlock **repo-aware checks**: dead commands the file references but `package.json` doesn't define, paths that don't exist, lines that just restate the README, unresolved `@`-imports, and a missing CLAUDE.md ↔ AGENTS.md bridge.

| Flag | Meaning |
|---|---|
| `--json` | machine-readable report on stdout |
| `--min <score>` | pass/fail threshold (default 60) |
| `--version` | print version |

### What a report looks like

Real output from `aispekt vague.md` (trimmed):

```
aispekt — vague.md (file mode)
76/100 (B)  ·  7 lines · ~48 tokens loaded every session · rulepack v1.0.1

  ERROR L   1  No runnable commands [measured]
        No runnable command found anywhere in the file.
        > # Guidelines
        fix: Add a commands section first: exact build, test, and lint
             invocations with flags, in backticks.
        evidence: https://arxiv.org/abs/2602.11988

  WARN  L   3  Vague / aspirational rule [official]
        Vague phrase "write clean code" — nothing testable for the agent.
        > Write clean code and follow best practices.
        fix: Replace with a concrete, verifiable instruction (exact command,
             path, or example) or delete the line.
        evidence: https://code.claude.com/docs/en/best-practices

  … 3 more findings …

  Claude Code:  Injects CLAUDE.md as an advisory user message (no compliance
                guarantee); official size target is under 200 lines.
  OpenAI Codex: Reads AGENTS.md natively (nearest file wins, 32 KiB cap).
  …
```

Every finding names the line, quotes it, proposes a concrete fix, and links its evidence.

### In CI

Exit codes: `0` when score ≥ min · `1` below min · `2` on usage/IO errors — drop it into CI or a pre-commit hook:

```sh
# fail the build when the instruction file degrades
bunx aispekt . --min 75
```

### Suppressing a finding

Heuristics have edges. Add an `aispekt-ignore` marker on the flagged line or the line above to suppress it — like `eslint-disable-next-line`:

```markdown
<!-- aispekt-ignore -->
- Release tags look like `shopnavigation/mainnavigationpath/1.2.3`
```

(Version-suffixed identifiers like git tags are already skipped automatically. The legacy `prune-ignore` marker also works.)

## Why penalty-only scoring?

The 2026 evidence flipped the old "write a thorough agents file" advice:

- Auto-generated/bloated context files **lowered** agent success and raised cost >20% (ETH Zurich, [arXiv:2602.11988](https://arxiv.org/abs/2602.11988)). Hand-written files helped (+4%) only when carrying non-redundant, repo-specific facts.
- File *structure* (length, position, architecture) showed **no measurable effect** on compliance in a 1,650-session factorial study ([arXiv:2605.10039](https://arxiv.org/abs/2605.10039)) — content correctness is what matters.
- The documented failure modes are subtractive: Lint Leakage (62% of files), Context Bloat (42%), Skill Leakage (35%) ([arXiv:2606.15828](https://arxiv.org/abs/2606.15828)).

So aispekt scores by **penalty only** — you start at 100 and lose points for content the evidence says hurts. It never rewards padding, and a lean 30-line file with exact commands and real gotchas scores 100.

## Every rule cites its evidence

The rules live in a versioned data file, [`rules/rulepack.json`](rules/rulepack.json), separate from the engine — each rule carries its weight, severity, confidence tier, and citation. Every finding in the CLI and playground links its evidence and shows its tier (`measured` > `official` > `community` > `heuristic`). Rules the studies contradict (like the 200-line size target) are labeled `heuristic` and framed as cost, not fact — honesty is a feature.

Browse the full rule table, rendered live from the rulepack, at [aispekt.erfindor.com](https://aispekt.erfindor.com#rules).

Calibration is regression-tested: known-good and known-bad fixtures pin score bounds, and property tests guarantee scoring stays penalty-only — appending bloat can never raise a score.

## Roadmap (v2 candidates)

- LLM-judge layer (bring-your-own-key) for semantic checks: "is this rule dead weight for a strong model?"
- Behavioral A/B tier: run an agent on sample tasks with/without the file — the only ground-truth measurement.
- Rulepack fetched from a registry at runtime (Semgrep-style) instead of bundled.

## Contributing

Bug reports and rule-evidence corrections are especially welcome — every rule is only as good as its citation. Development setup, architecture notes, and the rulepack-update process are in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
