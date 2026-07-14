export type Severity = "error" | "warn" | "info";
export type EvidenceTier = "measured" | "official" | "community" | "heuristic";
export type Grade = "A" | "B" | "C" | "D" | "F";

export interface RuleMeta {
  id: string;
  name: string;
  severity: Severity;
  weight: number;
  maxPenalty: number;
  evidenceTier: EvidenceTier;
  evidenceUrl: string;
  lastVerified: string;
  summary: string;
  recommendation: string;
  scope: "file" | "repo";
}

export interface RulePack {
  version: string;
  updated: string;
  rules: RuleMeta[];
}

/** Repo context: full path list plus contents of the files checks need. */
export interface RepoContext {
  paths: string[];
  contents: Record<string, string>;
  /** Path enumeration hit the cap — absence is no longer provable. */
  truncated?: boolean;
  /** Browser folder inputs omit symlinks entirely. */
  symlinksHidden?: boolean;
}

export interface AnalysisInput {
  fileName: string;
  content: string;
  repo?: RepoContext;
}

/** What a check implementation returns; metadata is joined from the rulepack. */
export interface RawFinding {
  line: number;
  excerpt: string;
  message: string;
  /** Overrides the rule's generic recommendation when the case needs its own fix text. */
  recommendation?: string;
}

export interface Finding extends RawFinding {
  ruleId: string;
  ruleName: string;
  severity: Severity;
  recommendation: string;
  evidenceUrl: string;
  evidenceTier: EvidenceTier;
}

export interface RulePenalty {
  ruleId: string;
  count: number;
  penalty: number;
}

export interface AgentNote {
  agent: string;
  note: string;
}

export interface Report {
  fileName: string;
  mode: "file" | "repo";
  score: number;
  grade: Grade;
  lineCount: number;
  byteSize: number;
  tokenEstimate: number;
  findings: Finding[];
  penalties: RulePenalty[];
  agentNotes: AgentNote[];
  /** Analysis-quality caveats (e.g. truncated walk) — never silent. */
  notices: string[];
  rulepackVersion: string;
  rulepackUpdated: string;
}

/** Shared parsed view of the analyzed file, passed to every check. */
export interface CheckContext {
  fileName: string;
  content: string;
  lines: string[];
  /** true where the line is prose (outside fenced code blocks) */
  prose: boolean[];
  /** all inline code spans and fenced block lines, with their line numbers */
  codeSpans: { line: number; text: string }[];
  repo?: RepoContext;
}

export type CheckFn = (ctx: CheckContext) => RawFinding[];
