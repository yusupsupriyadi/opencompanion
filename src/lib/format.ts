import type { CliKind, PermMode, SessionInfo, Status } from "./api";
import { i18n, t, tb, type Key } from "./i18n.svelte";

export const CLI_LABEL: Record<CliKind, string> = {
  claude: "Claude Code",
  codex: "Codex CLI",
  opencode: "OpenCode",
  gemini: "Gemini CLI",
  ccs: "CCS",
  pi: "Pi",
  omp: "omp",
};

/**
 * Chip class and label per status (DESIGN.md section 2, status table). Labels are getters, so a
 * template that reads one follows the UI language.
 */
export const STATUS: Record<Status, { chip: string; label: string; dot: string }> = {
  starting: { chip: "idle", dot: "run", get label() { return t("shell.status.starting"); } },
  running: { chip: "run", dot: "run", get label() { return t("shell.status.running"); } },
  waiting: { chip: "wait", dot: "wait", get label() { return t("shell.status.waiting"); } },
  idle: { chip: "idle", dot: "run", get label() { return t("shell.status.idle"); } },
  done: { chip: "idle", dot: "done", get label() { return t("shell.status.done"); } },
  error: { chip: "err", dot: "err", get label() { return t("shell.status.error"); } },
  stopped: { chip: "idle", dot: "done", get label() { return t("shell.status.stopped"); } },
};

const EFFORT_KEY: Record<string, Key> = {
  none: "shell.effort.none",
  minimal: "shell.effort.minimal",
  low: "shell.effort.low",
  medium: "shell.effort.medium",
  high: "shell.effort.high",
  xhigh: "shell.effort.xhigh",
  max: "shell.effort.max",
  ultra: "shell.effort.ultra",
};

/** A thinking level as the pickers show it; a level the app does not know yet keeps its CLI name. */
export function effortLabel(effort: string): string {
  const key = EFFORT_KEY[effort];
  return key ? t(key) : effort;
}

/** Dates and times follow the Indonesian UI; in English they keep the system's format. */
function locale(): string | undefined {
  return i18n.lang === "id" ? "id-ID" : undefined;
}

export function isLive(s: Pick<SessionInfo, "status">): boolean {
  return s.status === "starting" || s.status === "running" || s.status === "waiting" || s.status === "idle";
}

export function folderName(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Shortens the home folder to `~` for display; the full path stays in titles and tooltips. */
export function shortPath(path: string): string {
  const m = path.match(/^[A-Za-z]:[\\/]Users[\\/][^\\/]+/);
  return m ? "~" + path.slice(m[0].length).replace(/\\/g, "/") : path;
}

export function duration(ms: number): string {
  const mins = Math.max(0, Math.floor(ms / 60000));
  if (mins < 1) return t("shell.time.underMinute");
  if (mins < 60) return t("shell.time.min", { n: mins });
  return t("shell.time.hMin", { h: Math.floor(mins / 60), m: mins % 60 });
}

/** Memory as Task Manager words it: MB below a gigabyte, then GB with one decimal. */
export function memory(bytes: number): string {
  const mb = bytes / 1048576;
  return mb < 1024 ? `${Math.round(mb)} MB` : `${(mb / 1024).toFixed(1)} GB`;
}

export function ago(at: number, now = Date.now()): string {
  const mins = Math.floor((now - at) / 60000);
  if (mins < 1) return t("shell.time.justNow");
  if (mins < 60) return t("shell.time.minAgo", { n: mins });
  const h = Math.floor(mins / 60);
  if (h < 24) return t("shell.time.hAgo", { n: h });
  return new Date(at).toLocaleDateString(locale(), { day: "numeric", month: "short" });
}

export function clock(at: number): string {
  return new Date(at).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
}

export function isToday(at: number): boolean {
  const d = new Date(at);
  const n = new Date();
  return d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate();
}

/** One line under a session row: last event plus how long it ran. */
export function trackLine(s: SessionInfo, now = Date.now()): string {
  const took = duration((s.endedAt ?? now) - s.startedAt);
  if (s.status === "done") return t("shell.track.finished", { took });
  if (s.status === "stopped") return `${s.lastEvent ? tb(s.lastEvent) : t("shell.status.stopped")} · ${took}`;
  if (s.status === "error") return `${s.lastEvent ? tb(s.lastEvent) : t("shell.status.error")} · ${took}`;
  return `${s.lastEvent ? tb(s.lastEvent) : t("shell.status.starting")} · ${took}`;
}

export function waitingTitle(s: SessionInfo): string {
  const cli = CLI_LABEL[s.cli];
  const w = s.waiting;
  if (!w) return t("shell.waiting.default", { cli });
  if (w.reason === "trust_folder") return t("shell.waiting.trustFolder", { cli });
  if (w.reason === "update_offer") return t("shell.waiting.updateOffer", { cli });
  if (w.tool === "Bash" || w.tool === "PowerShell") return t("shell.waiting.command", { cli });
  if (w.tool === "Write" || w.tool === "Edit" || w.tool === "MultiEdit") return t("shell.waiting.file", { cli });
  if (w.tool) return t("shell.waiting.tool", { cli, tool: w.tool });
  return t("shell.waiting.permission", { cli });
}

/**
 * Permission modes for sessions OpenCompanion starts. `flags` mirrors `headless::mode_flags` in
 * src-tauri, which was checked against each CLI's --help. The words are getters that follow the UI
 * language; `flags` describe the CLI's own flags and stay in English.
 */
export const MODES: {
  id: PermMode;
  label: string;
  short: string;
  detail: string;
  flags: Record<"claude" | "codex" | "opencode" | "pi" | "omp", string>;
}[] = [
  {
    id: "ask",
    get label() { return t("shell.mode.ask.label"); },
    get short() { return t("shell.mode.ask.short"); },
    get detail() { return t("shell.mode.ask.detail"); },
    flags: { claude: "--permission-mode manual", codex: "sandbox workspace-write, asks on request", opencode: "its own rules; asks in the terminal", pi: "never asks; every tool runs", omp: "--approval-mode always-ask" },
  },
  {
    id: "plan",
    get label() { return t("shell.mode.plan.label"); },
    get short() { return t("shell.mode.plan.short"); },
    get detail() { return t("shell.mode.plan.detail"); },
    flags: { claude: "--permission-mode plan", codex: "sandbox read-only", opencode: "--agent plan", pi: "--tools read,grep,find,ls", omp: "--tools read,grep,glob --approval-mode always-ask" },
  },
  {
    id: "auto",
    get label() { return t("shell.mode.auto.label"); },
    get short() { return t("shell.mode.auto.short"); },
    get detail() { return t("shell.mode.auto.detail"); },
    flags: { claude: "--permission-mode auto", codex: "--approve-for-me", opencode: "--auto", pi: "never asks; every tool runs", omp: "--approval-mode write" },
  },
  {
    id: "bypass",
    get label() { return t("shell.mode.bypass.label"); },
    get short() { return t("shell.mode.bypass.short"); },
    get detail() { return t("shell.mode.bypass.detail"); },
    flags: { claude: "--dangerously-skip-permissions", codex: "--dangerously-bypass-approvals-and-sandbox", opencode: "--auto, every permission allowed", pi: "never asks; every tool runs", omp: "--approval-mode yolo" },
  },
];

export function modeLabel(id: string | null | undefined): string {
  return MODES.find((m) => m.id === id)?.label ?? t("shell.mode.ask.label");
}

/** Text sizes in Settings, in percent. Mirrors `db::TEXT_SIZES` in src-tauri. */
export const TEXT_SIZES = [90, 100, 110, 125, 150] as const;

export const SIGNAL_TEXT: Record<string, string> = {
  get stdio() { return t("shell.signal.stdio"); },
  get hook() { return t("shell.signal.hook"); },
  get screen() { return t("shell.signal.screen"); },
};
