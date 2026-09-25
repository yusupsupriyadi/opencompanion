import type { CliKind, PermMode, SessionInfo, Status } from "./api";

export const CLI_LABEL: Record<CliKind, string> = {
  claude: "Claude Code",
  codex: "Codex CLI",
  opencode: "OpenCode",
  gemini: "Gemini CLI",
};

/** Chip class and label per status (DESIGN.md section 2, status table). */
export const STATUS: Record<Status, { chip: string; label: string; dot: string }> = {
  starting: { chip: "idle", label: "Starting", dot: "run" },
  running: { chip: "run", label: "Running", dot: "run" },
  waiting: { chip: "wait", label: "Waiting for you", dot: "wait" },
  idle: { chip: "idle", label: "Idle", dot: "run" },
  done: { chip: "idle", label: "Done", dot: "done" },
  error: { chip: "err", label: "Error", dot: "err" },
  stopped: { chip: "idle", label: "Stopped", dot: "done" },
};

const EFFORT_LABEL: Record<string, string> = {
  none: "None",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra high",
  max: "Max",
  ultra: "Ultra",
};

/** A thinking level as the pickers show it; a level the app does not know yet keeps its CLI name. */
export function effortLabel(effort: string): string {
  return EFFORT_LABEL[effort] ?? effort;
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
  if (mins < 1) return "under a minute";
  if (mins < 60) return `${mins} min`;
  const h = Math.floor(mins / 60);
  return `${h} h ${mins % 60} min`;
}

export function ago(at: number, now = Date.now()): string {
  const mins = Math.floor((now - at) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const h = Math.floor(mins / 60);
  if (h < 24) return `${h} h ago`;
  return new Date(at).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

export function clock(at: number): string {
  return new Date(at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function isToday(at: number): boolean {
  const d = new Date(at);
  const n = new Date();
  return d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate();
}

/** One line under a session row: last event plus how long it ran. */
export function trackLine(s: SessionInfo, now = Date.now()): string {
  const took = duration((s.endedAt ?? now) - s.startedAt);
  if (s.status === "done") return `Finished · took ${took}`;
  if (s.status === "stopped") return `${s.lastEvent ?? "Stopped"} · ${took}`;
  if (s.status === "error") return `${s.lastEvent ?? "Error"} · ${took}`;
  return `${s.lastEvent ?? "Starting"} · ${took}`;
}

export function waitingTitle(s: SessionInfo): string {
  const who = CLI_LABEL[s.cli];
  const w = s.waiting;
  if (!w) return `${who} is waiting for you`;
  if (w.reason === "trust_folder") return `${who} asks whether you trust this folder`;
  if (w.reason === "update_offer") return `${who} is offering to update itself`;
  if (w.tool === "Bash" || w.tool === "PowerShell") return `${who} wants to run a command`;
  if (w.tool === "Write" || w.tool === "Edit" || w.tool === "MultiEdit") return `${who} wants to change a file`;
  if (w.tool) return `${who} wants to use ${w.tool}`;
  return `${who} needs your permission`;
}

/**
 * Permission modes for sessions AI Remote starts. `flags` mirrors `headless::mode_flags` in
 * src-tauri, which was checked against each CLI's --help.
 */
export const MODES: {
  id: PermMode;
  label: string;
  short: string;
  detail: string;
  flags: Record<"claude" | "codex" | "opencode", string>;
}[] = [
  {
    id: "ask",
    label: "Ask me",
    short: "Every permission prompt comes to you.",
    detail: "The CLI stops and waits before commands and file changes. The request shows up under Needs you, on the desktop and on your phone.",
    flags: { claude: "--permission-mode manual", codex: "sandbox workspace-write, asks on request", opencode: "its own rules; asks in the terminal" },
  },
  {
    id: "plan",
    label: "Plan",
    short: "Reads and plans, changes nothing.",
    detail: "Good for questions, reviews and working out an approach before any edit.",
    flags: { claude: "--permission-mode plan", codex: "sandbox read-only", opencode: "--agent plan" },
  },
  {
    id: "auto",
    label: "Auto",
    short: "The CLI approves routine actions itself.",
    detail: "Fewer interruptions. Each CLI decides what counts as routine, and risky actions can still ask.",
    flags: { claude: "--permission-mode auto", codex: "--approve-for-me", opencode: "--auto" },
  },
  {
    id: "bypass",
    label: "Bypass",
    short: "No permission checks at all.",
    detail: "The CLI can change or delete any file and run any command without asking. Use it only in a folder you can afford to lose, or inside a sandbox.",
    flags: { claude: "--dangerously-skip-permissions", codex: "--dangerously-bypass-approvals-and-sandbox", opencode: "--auto, every permission allowed" },
  },
];

export function modeLabel(id: string | null | undefined): string {
  return MODES.find((m) => m.id === id)?.label ?? "Ask me";
}

/** Text sizes in Settings, in percent. Mirrors `db::TEXT_SIZES` in src-tauri. */
export const TEXT_SIZES = [90, 100, 110, 125, 150] as const;

export const SIGNAL_TEXT: Record<string, string> = {
  stdio: "Permission requests arrive through Claude Code's control protocol, so Approve and Deny answer it directly.",
  hook: "Claude Code reports permission prompts through hooks added to this session only.",
  screen: "Detected from the terminal screen. Text patterns are a fallback and can miss a prompt.",
};
