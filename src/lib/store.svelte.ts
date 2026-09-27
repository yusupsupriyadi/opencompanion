import { listen } from "@tauri-apps/api/event";
import {
  api,
  errorText,
  type CliInstall,
  type CompanionStatus,
  type EventRow,
  type ExternalSession,
  type ModelList,
  type SessionInfo,
  type SessionView,
  type Settings,
} from "./api";
import { setLang } from "./i18n.svelte";

type LoadState = "loading" | "ready" | "error";

/** Session events that put a mark on the horizon track. */
export const MARK_KINDS = new Set(["tool_call", "file_changed", "permission_request", "permission_denied", "tool_failed", "error"]);

/** Desktop app state, fed by backend events so every screen shows the same sessions. */
export const app = $state({
  sessions: [] as SessionView[],
  sessionsState: "loading" as LoadState,
  sessionsError: "",
  clis: [] as CliInstall[],
  clisState: "loading" as LoadState,
  clisError: "",
  checkingClis: false,
  outside: [] as ExternalSession[],
  outsideState: "loading" as LoadState,
  outsideError: "",
  scanning: false,
  settings: null as Settings | null,
  companion: null as CompanionStatus | null,
  now: Date.now(),
});

export async function refreshSessions() {
  try {
    app.sessions = await api.listSessions(80);
    app.sessionsState = "ready";
  } catch (e) {
    app.sessionsState = "error";
    app.sessionsError = errorText(e);
  }
}

export async function refreshClis() {
  if (app.checkingClis) return;
  app.checkingClis = true;
  forgetModelLists();
  try {
    app.clis = await api.detectClis();
    app.clisState = "ready";
  } catch (e) {
    app.clisState = "error";
    app.clisError = errorText(e);
  } finally {
    app.checkingClis = false;
  }
}

export async function refreshOutside() {
  if (app.scanning) return;
  app.scanning = true;
  try {
    app.outside = await api.scanExternal();
    app.outsideState = "ready";
  } catch (e) {
    app.outsideState = "error";
    app.outsideError = errorText(e);
  } finally {
    app.scanning = false;
  }
}

// Model lists per CLI and version. Codex and OpenCode take seconds to print theirs, so a list is
// asked for once; a failed one is asked for again next time.
const modelLists = new Map<string, Promise<ModelList>>();

export function plannerModels(cli: CliInstall): Promise<ModelList> {
  const key = `${cli.kind}@${cli.version ?? ""}`;
  let list = modelLists.get(key);
  if (!list) {
    list = api.chatModels(cli.kind);
    modelLists.set(key, list);
    list.catch(() => modelLists.delete(key));
  }
  return list;
}

/** Rescanning the CLIs asks for fresh lists, for example after a provider was added to OpenCode. */
export function forgetModelLists() {
  modelLists.clear();
}

export async function loadSettings() {
  app.settings = await api.getSettings();
  setLang(app.settings?.language);
  app.companion = await api.companionStatus();
  return app.settings;
}

export async function saveSettings(next: Settings) {
  app.settings = await api.saveSettings(next);
  setLang(app.settings?.language);
  app.companion = await api.companionStatus();
  restartScanTimer();
  return app.settings;
}

function upsert(info: SessionInfo) {
  const i = app.sessions.findIndex((s) => s.id === info.id);
  if (i >= 0) app.sessions[i] = { ...app.sessions[i], ...info };
  else app.sessions.unshift({ ...info, marks: [] });
}

function addMark(row: EventRow) {
  if (!MARK_KINDS.has(row.event.kind)) return;
  const s = app.sessions.find((x) => x.id === row.sessionId);
  const mark: [number, string] = [row.at, row.event.kind];
  if (s) s.marks = [mark, ...s.marks].slice(0, 40);
}

function forget(id: string) {
  app.sessions = app.sessions.filter((s) => s.id !== id);
}

/** Only finished sessions can be deleted; the backend refuses a live one. */
export async function deleteSession(id: string) {
  await api.deleteSession(id);
  forget(id);
}

/** The session the delete dialog asks about. The shell renders that dialog once for every list. */
export const pendingDelete = $state({ session: null as SessionView | null });
export function askDelete(s: SessionView) {
  pendingDelete.session = s;
}

/** The New session dialog, also rendered once by the shell. `cwd` pre-fills the folder, or stays empty to choose one. */
export const pendingNew = $state({ open: false, cwd: "" });
export function askNewSession(cwd = "") {
  pendingNew.cwd = cwd;
  pendingNew.open = true;
}

let started = false;
let scanTimer: ReturnType<typeof setInterval> | undefined;

function restartScanTimer() {
  if (scanTimer) clearInterval(scanTimer);
  const secs = Math.max(3, app.settings?.scanSeconds ?? 10);
  scanTimer = setInterval(refreshOutside, secs * 1000);
}

/** Called once by the desktop shell. The phone routes never call it: they have no Tauri. */
export async function startDesktop() {
  if (started) return;
  started = true;
  await listen<SessionInfo>("session-updated", (e) => upsert(e.payload));
  await listen<EventRow>("session-event", (e) => addMark(e.payload));
  await listen<string>("session-deleted", (e) => forget(e.payload));
  // The phone changed a setting, such as the planner's model.
  await listen("settings-changed", () => loadSettings().catch(() => undefined));
  refreshSessions();
  refreshClis();
  refreshOutside();
  try {
    await loadSettings();
  } catch {
    // Settings fall back to defaults in the screens that need them.
  }
  restartScanTimer();
  setInterval(() => (app.now = Date.now()), 30_000);
}

// Toast feedback, announced through a polite live region in the shell.
export const toast = $state({ text: "", show: false });
let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function showToast(text: string) {
  toast.text = text;
  toast.show = true;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.show = false), 3400);
}

// Theme: follows the OS until the user picks Day or Dusk (PRD FR-61).
export type Theme = "light" | "dark";
function readTheme(): Theme | null {
  try {
    const t = localStorage.getItem("air-theme");
    return t === "light" || t === "dark" ? t : null;
  } catch {
    return null;
  }
}
export const theme = $state({ chosen: readTheme() as Theme | null, system: "light" as Theme });

export function initTheme() {
  const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
  theme.system = mq?.matches ? "dark" : "light";
  mq?.addEventListener?.("change", (e) => (theme.system = e.matches ? "dark" : "light"));
  applyTheme();
}

export function currentTheme(): Theme {
  return theme.chosen ?? theme.system;
}

export function setTheme(t: Theme) {
  theme.chosen = t;
  try {
    localStorage.setItem("air-theme", t);
  } catch {
    // Private windows can refuse storage; the choice still holds for this session.
  }
  applyTheme();
}

function applyTheme() {
  if (theme.chosen) document.documentElement.dataset.theme = theme.chosen;
  else delete document.documentElement.dataset.theme;
}
