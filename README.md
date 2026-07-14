# 🔍 aispekt

**Is your AGENTS.md / CLAUDE.md helping your coding agent — or silently taxing it?**

Drop an instruction file (or your whole repo folder) and get a deterministic score plus evidence-cited recommendations: what to cut, what to fix, and the study or official doc behind every finding. 100% client-side — nothing is uploaded.

## Why subtractive?

The 2026 evidence flipped the old "write a thorough agents file" advice:

- Auto-generated/bloated context files **lowered** agent success and raised cost >20% (ETH Zurich, [arXiv:2602.11988](https://arxiv.org/abs/2602.11988)). Hand-written files helped (+4%) only when carrying non-redundant, repo-specific facts.
- File *structure* (length, position, architecture) showed **no measurable effect** on compliance in a 1,650-session factorial study ([arXiv:2605.10039](https://arxiv.org/abs/2605.10039)) — content correctness is what matters.
- The documented failure modes are subtractive: Lint Leakage (62% of files), Context Bloat (42%), Skill Leakage (35%) ([arXiv:2606.15828](https://arxiv.org/abs/2606.15828)).

So aispekt scores by **penalty only** — it never rewards padding, and a lean 30-line file with exact commands and real gotchas scores 100.

## Usage

**Web:** `bun run dev`, then drop a file or folder onto the page. Folder drops unlock repo-aware checks (dead commands vs package.json, stale paths, README redundancy, missing CLAUDE.md bridge, unresolved @-imports).

**CLI:** same engine, real filesystem:

```
bunx aispekt <file-or-dir>  # or locally: bun src/cli.ts <file-or-dir> [--json] [--min <score>]
```

Exit codes: 0 when score ≥ min (default 60), 1 below, 2 on usage/IO errors — drop it into CI or a pre-commit hook.

## How it stays current (the rulepack)

The ecosystem moves fast; hardcoded rules rot. aispekt separates the **engine** (check implementations, TypeScript) from the **rulepack** (`rules/rulepack.json`) — the versioned data that carries each rule's weight, severity, confidence tier, and citation:

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
2. New rule → add the JSON entry *and* a check implementation; the engine **throws at load** if pack and implementations disagree in either direction, so drift is impossible to ship.
3. Every finding in the UI/CLI links its `evidenceUrl` and shows its tier (`measured` > `official` > `community` > `heuristic`). Rules the studies contradict (like the 200-line target) are labeled `heuristic` and framed as cost, not adherence — honesty is a feature.
4. Re-verify cadence: sweep the evidence URLs (agents.md spec, Anthropic memory/best-practices docs, the arXiv studies) and refresh `lastVerified`; both doc hosts moved in the last year, so follow redirects.

Calibration is regression-tested: known-good and known-bad fixtures pin score bounds (`test/calibration.test.ts`), and property tests guarantee scoring stays penalty-only (appending bloat can never raise a score).

## Suppressing a finding

Heuristics have edges. Add an `aispekt-ignore` marker (the legacy `prune-ignore` also works) on the flagged line or the line above to suppress it:

```markdown
<!-- aispekt-ignore -->
- Release tags look like `shopnavigation/mainnavigationpath/1.2.3`
```

(Version-suffixed identifiers like git tags are already skipped automatically.)

## Roadmap (v2 candidates)

- LLM-judge layer (bring-your-own-key) for semantic checks: "is this rule dead weight for a strong model?"
- Behavioral A/B tier: run an agent on sample tasks with/without the file — the only ground-truth measurement.
- Rulepack fetched from a registry at runtime (Semgrep-style) instead of bundled.

## Development

```
bun install
bun test           # 55+ tests incl. 1000-run property suites
bun run typecheck
bun run build      # tsc + vite
```

Zero runtime dependencies. The engine (`src/engine/`) is pure TS with no DOM — the web app and CLI are thin shells over it.
