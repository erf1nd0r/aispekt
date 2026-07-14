import type { Report } from "../engine/types";

function esc(s: string): string {
  return s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

const TIER_LABEL: Record<string, string> = {
  measured: "measured study",
  official: "official docs",
  community: "community",
  heuristic: "heuristic",
};

export function renderReport(r: Report): string {
  const findingsHtml =
    r.findings.length === 0
      ? `<div class="clean">Nothing to cut — this is a lean file. ✂️</div>`
      : `<div class="findings">${r.findings
          .map(
            (f) => `
      <article class="finding sev-${f.severity}">
        <div class="head">
          <span class="sev">${f.severity}</span>
          <span class="rule">${esc(f.ruleName)}</span>
          <span class="lineno">L${f.line}</span>
          <a class="tier ${f.evidenceTier}" href="${esc(f.evidenceUrl)}" target="_blank" rel="noopener"
             title="Open the evidence behind this rule">${TIER_LABEL[f.evidenceTier] ?? f.evidenceTier} ↗</a>
        </div>
        <p class="msg">${esc(f.message)}</p>
        ${f.excerpt ? `<div class="excerpt">${esc(f.excerpt)}</div>` : ""}
        <p class="fix"><b>Fix:</b> ${esc(f.recommendation)}</p>
      </article>`,
          )
          .join("")}</div>`;

  const agentsHtml = `
    <section class="agents">
      <h3>How the major agents will treat this file</h3>
      ${r.agentNotes
        .map(
          (n) => `
        <div class="agent-note">
          <span class="who">${esc(n.agent)}</span>
          <span class="what">${esc(n.note)}</span>
        </div>`,
        )
        .join("")}
    </section>`;

  return `
    <div class="scorecard grade-${r.grade}">
      <div class="score-badge">
        <span class="num">${r.score}</span>
        <span class="grade">grade ${r.grade}</span>
      </div>
      <div class="scoremeta">
        <h2>${esc(r.fileName)}</h2>
        <p class="facts">
          ${r.lineCount} lines · ${(r.byteSize / 1024).toFixed(1)} KiB ·
          ~${r.tokenEstimate.toLocaleString()} tokens loaded into <em>every</em> agent session ·
          ${r.findings.length} finding${r.findings.length === 1 ? "" : "s"}
        </p>
        <span class="mode-chip">${
          r.mode === "repo"
            ? "repo mode — checked against your actual repo"
            : "file mode — drop the whole repo folder for drift + redundancy checks"
        }</span>
      </div>
    </div>
    ${r.notices
      .map((n) => `<div class="error-banner" role="note">⚠️ ${esc(n)}</div>`)
      .join("")}
    ${findingsHtml}
    ${agentsHtml}`;
}

export function renderError(message: string): string {
  return `<div class="error-banner">${esc(message)}</div>`;
}
