---
name: aispekt-judge
description: Semantic judge pass for AGENTS.md/CLAUDE.md instruction files, layered on aispekt's deterministic score. The agent running this skill IS the judge — no API key involved. USE WHEN asked to judge, semantically review, or deep-audit an agent instruction file with aispekt, or after an aispekt run when the user wants the semantic layer. Requires the aispekt CLI (bunx aispekt / npm i -g aispekt / cargo install aispekt-cli).
---

# aispekt judge — you are the judge

aispekt's deterministic engine scores what pattern-matching can prove, and that score is final. This skill runs the layer above it: the questions only a model can answer (dead weight, semantic vagueness, contradictions, stale guidance). You do the judging; the CLI keeps the bookkeeping honest.

## Workflow

1. Emit the brief:

   ```
   aispekt judge emit <file-or-dir> --out aispekt-brief.json
   ```

2. Read `aispekt-brief.json`. It is self-contained: the instruction-file content is embedded, `tasks` lists one semantic check each with its question and guidance, and `answersContract` is the exact shape your answers file must have.

3. Judge every task against the embedded `content` only — not your memory of the repo, not the file on disk (it may have changed since the brief). For each task produce findings, or an empty array if the check passes.

4. Write `aispekt-answers.json` following `answersContract.shape` exactly. Copy `contentHash` verbatim from `target.contentHash`. Set `judge.agent` and `judge.model` to what you actually are — never impersonate another judge.

5. Merge and render:

   ```
   aispekt judge merge aispekt-brief.json aispekt-answers.json
   ```

6. Show the rendered output to the user unedited, then add your own summary if useful. If `merge` exits 2, the error names what is wrong with your answers file — fix it and re-run; never edit the brief.

7. The two JSON files are working artifacts. Ask the user, or follow their conventions, on whether to keep or delete them.

## Judging rules

- Flag only what you can quote: every finding carries a verbatim excerpt and real 1-based line numbers from the embedded content.
- An empty findings array is a first-class answer. Do not pad findings to look thorough — a clean file is clean.
- Calibrate `confidence` honestly: `high` means you would defend it in review; `low` means worth a human look.
- Never flag exact commands, non-obvious gotchas, safety gates, or output-format contracts as dead weight.
- The deterministic score is not yours to change, restate as changed, or average with your verdicts.
