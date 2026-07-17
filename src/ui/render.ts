import type { Report, Severity } from "../engine/types";

export function esc(s: string): string {
  return s
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export const TIER_LABEL: Record<string, string> = {
  measured: "measured study",
  official: "official docs",
  community: "community",
  heuristic: "heuristic",
};

/**
 * Grade badge as a stamped inspection seal: double ring, micro-text on a
 * circular path, score-proportional gauge stroke, grade embossed center.
 * Pure SVG — color keying comes from the .scorecard grade class.
 */
export function renderSeal(score: number, grade: string, rulepackVersion: string): string {
  const r = 54;
  const circumference = 2 * Math.PI * r;
  const filled = (circumference * score) / 100;
  const micro = `INSPECTED · AISPEKT · RULEPACK V${rulepackVersion} · SCORE ${score}/100 · `;
  return `
    <svg class="seal" viewBox="0 0 120 120" role="img" aria-label="Score ${score} of 100, grade ${esc(grade)}">
      <defs>
        <path id="seal-ring" d="M 60,9 a 51,51 0 1,1 -0.01,0" />
      </defs>
      <circle class="seal-track" cx="60" cy="60" r="${r}" />
      <circle class="seal-gauge" cx="60" cy="60" r="${r}"
        stroke-dasharray="${filled.toFixed(2)} ${circumference.toFixed(2)}"
        transform="rotate(-90 60 60)" />
      <circle class="seal-inner" cx="60" cy="60" r="40" />
      <text class="seal-micro"><textPath href="#seal-ring">${esc(micro)}</textPath></text>
      <text class="seal-grade" x="60" y="63" text-anchor="middle">${esc(grade)}</text>
      <text class="seal-num" x="60" y="82" text-anchor="middle">${score}/100</text>
    </svg>`;
}

/** Penalty ledger: a 100-unit hairline bar showing where the points went. */
function renderLedger(r: Report): string {
  if (r.penalties.length === 0) return "";
  const sevOf = new Map<string, Severity>();
  for (const f of r.findings) sevOf.set(f.ruleId, f.severity);
  const segments = r.penalties
    .map((p) => {
      const sev = sevOf.get(p.ruleId) ?? "info";
      return `<a class="ledger-seg sev-${sev}" href="#finding-${esc(p.ruleId)}"
        style="width:${Math.max(p.penalty, 0.6)}%"
        title="${esc(p.ruleId)}: −${p.penalty} (${p.count} finding${p.count === 1 ? "" : "s"})"></a>`;
    })
    .join("");
  return `
    <div class="ledger" aria-label="Penalty breakdown">
      <div class="ledger-bar">${segments}<span class="ledger-rest"></span></div>
      <span class="ledger-caption">−${(100 - r.score)} points · hover a segment, click to jump</span>
    </div>`;
}

/** Finding as an annotated manuscript margin: source line left, note right. */
function renderFinding(f: Report["findings"][number], anchorUsed: Set<string>): string {
  const anchor = anchorUsed.has(f.ruleId) ? "" : ` id="finding-${esc(f.ruleId)}"`;
  anchorUsed.add(f.ruleId);
  return `
      <article class="finding sev-${f.severity}"${anchor}>
        <div class="finding-source">
          <span class="lineno">L${f.line}</span>
          ${f.excerpt ? `<code class="excerpt">${esc(f.excerpt)}</code>` : `<span class="excerpt none">—</span>`}
        </div>
        <div class="finding-note">
          <div class="head">
            <span class="sev">${f.severity}</span>
            <span class="rule">${esc(f.ruleName)}</span>
            <a class="tier ${f.evidenceTier}" href="${esc(f.evidenceUrl)}" target="_blank" rel="noopener"
               title="Open the evidence behind this rule">${TIER_LABEL[f.evidenceTier] ?? f.evidenceTier} ↗</a>
          </div>
          <p class="msg">${esc(f.message)}</p>
          <p class="fix"><b>Fix:</b> ${esc(f.recommendation)}</p>
        </div>
      </article>`;
}

export function renderReport(r: Report): string {
  const anchorUsed = new Set<string>();
  const findingsHtml =
    r.findings.length === 0
      ? `<div class="clean">Inspection complete — nothing to cut. Lean file.</div>`
      : `<div class="findings">${r.findings.map((f) => renderFinding(f, anchorUsed)).join("")}</div>`;

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
      ${renderSeal(r.score, r.grade, r.rulepackVersion)}
      <div class="scoremeta">
        <h2>${esc(r.fileName)}</h2>
        <p class="facts">
          ${r.lineCount} lines · ${(r.byteSize / 1024).toFixed(1)} KiB ·
          ~${r.tokenEstimate.toLocaleString()} tokens loaded into <em>every</em> agent session ·
          ${r.findings.length} finding${r.findings.length === 1 ? "" : "s"}
        </p>
        ${renderLedger(r)}
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
