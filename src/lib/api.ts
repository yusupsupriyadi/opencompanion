import { invoke } from "@tauri-apps/api/core";

export type CliKind = "claude" | "codex" | "opencode" | "gemini" | "ccs" | "pi" | "omp";
export type Mode = "interactive" | "headless";
export type Status = "starting" | "running" | "waiting" | "idle" | "shell" | "done" | "error" | "stopped";
export type PermMode = "ask" | "plan" | "auto" | "bypass";

/** Mirrors `cli::CliInstall`. */
export interface CliInstall {
  kind: CliKind;
  label: string;
  path: string | null;
  version: string | null;
  tested: boolean;
  error: string | null;
}

/** Mirrors `monitor::ExternalSession`. */
export interface ExternalSession {
  pid: number;
  kind: CliKind;
  mode: "interactive" | "headless" | "server";
  cwd: string | null;
  startedAt: number;
  cpuPercent: number;
  memoryBytes: number;
}

/** Mirrors `monitor::Usage`: a session's CLI and every process it started. */
export interface Usage {
  /** Share of the whole machine, as Task Manager counts it. */
  cpuPercent: number;
  memoryBytes: number;
  children: number;
}

/** Mirrors `transcript::Line`. */
export interface TranscriptLine {
  speaker: "you" | "cli" | "tool";
  text: string;
  at: number | null;
}

/** Mirrors `transcript::Transcript`: the last messages a CLI opened outside OpenCompanion wrote to its own history. */
export interface Transcript {
  source: string | null;
  lines: TranscriptLine[];
  /** Why there are no lines, in words for the owner. */
  note: string | null;
}

/** Mirrors `skills::SkillRoot`. */
export interface SkillRoot {
  id: string;
  label: string;
  cli: CliKind | null;
  path: string;
  exists: boolean;
}

export type SkillProblem = "noSkillMd" | "brokenLink" | "unreadable" | "tooLarge";

/** Mirrors `skills::SkillEntry`. */
export interface SkillEntry {
  rootId: string;
  path: string;
  hash: string | null;
  variant: string | null;
  modifiedAt: number | null;
  linkTarget: string | null;
  containsLinks: boolean;
  problem: SkillProblem | null;
}

/** Mirrors `skills::SkillRow`. */
export interface SkillRow {
  name: string;
  description: string | null;
  variants: number;
  entries: SkillEntry[];
}

/** Mirrors `skills::SkillScan`. */
export interface SkillScan {
  shell: "powershell" | "sh";
  roots: SkillRoot[];
  skills: SkillRow[];
}

/** Mirrors `db::Waiting`. */
export interface Waiting {
  reason: string;
  tool: string | null;
  detail: string;
  requestId: string | null;
  canAnswer: boolean;
  method: "stdio" | "hook" | "screen";
  since: number;
}

/** Mirrors `db::SessionInfo`. */
export interface SessionInfo {
  id: string;
  cli: CliKind;
  cwd: string;
  mode: Mode;
  title: string;
  prompt: string;
  status: Status;
  pid: number | null;
  cliSessionId: string | null;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  lastEvent: string | null;
  waiting: Waiting | null;
  source: string;
  permissionMode: string | null;
  updatedAt: number;
}

/** Mirrors `db::SessionQuery`: the history's search. Empty text and null filters match everything. */
export interface SessionQuery {
  text: string;
  cli: CliKind | null;
  folder: string | null;
  /** `live`, `waiting`, `done`, `error` or `stopped`. */
  status: string | null;
  limit: number;
  offset: number;
}

export interface SessionView extends SessionInfo {
  /** `[at, kind]`, newest first. */
  marks: [number, string][];
}

/** Mirrors `events::SessionEvent`. */
export type SessionEvent =
  | { kind: "started"; session_id: string }
  | { kind: "message"; text: string }
  | { kind: "tool_call"; tool: string; summary: string }
  | { kind: "tool_failed"; tool: string; message: string }
  | { kind: "file_changed"; path: string }
  | { kind: "permission_request"; request_id: string; tool: string; summary: string }
  | { kind: "permission_denied"; tool: string; summary: string }
  | { kind: "retrying"; message: string }
  | { kind: "done"; ok: boolean; summary: string }
  | { kind: "error"; message: string }
  | { kind: "raw"; line: string };

export interface EventRow {
  id: number;
  sessionId: string;
  at: number;
  event: SessionEvent;
}

export interface SessionDetail {
  session: SessionInfo;
  events: EventRow[];
}

/** Mirrors `files::Entry`. Paths are relative to the session folder, with `/` between parts. */
export interface FolderEntry {
  name: string;
  path: string;
  dir: boolean;
  ignored: boolean;
}

/** Mirrors `files::Listing`. */
export interface FolderListing {
  entries: FolderEntry[];
  truncated: boolean;
}

/** Mirrors `files::Match`: a file for a name search, a line of it for a text search. */
export interface FindMatch {
  path: string;
  line: number | null;
  text: string | null;
}

/** Mirrors `files::Found`. */
export interface FindResult {
  matches: FindMatch[];
  truncated: boolean;
}

/** Mirrors `files::FileText`. */
export interface FileText {
  text: string | null;
  size: number;
  binary: boolean;
  tooLarge: boolean;
}

export type ChangeCode = "M" | "A" | "D" | "R" | "C" | "U" | "?";

/** Mirrors `git::Change`. */
export interface GitChange {
  path: string;
  oldPath: string | null;
  code: ChangeCode;
  staged: boolean;
  unstaged: boolean;
  added: number | null;
  removed: number | null;
}

/** Mirrors `git::Status`: uncommitted changes in the session folder against HEAD. */
export interface GitStatus {
  repo: boolean;
  branch: string | null;
  head: string | null;
  upstream: string | null;
  ahead: number;
  behind: number;
  changes: GitChange[];
  truncated: boolean;
}

/** Mirrors `git::Diff`. */
export interface GitDiff {
  patch: string;
  binary: boolean;
  tooLarge: boolean;
}

/** Mirrors `git::Branch`. */
export interface GitBranch {
  name: string;
  current: boolean;
  upstream: string | null;
  ahead: number;
  behind: number;
  gone: boolean;
  subject: string;
  at: number;
}

/** Mirrors `git::Commit`. */
export interface GitCommit {
  hash: string;
  subject: string;
  author: string;
  at: number;
}

/** Mirrors `git::Branches`. */
export interface GitBranches {
  repo: boolean;
  branches: GitBranch[];
  commits: GitCommit[];
}

export interface DispatchCard {
  id: string;
  cli: CliKind;
  /** The task's name; empty on cards from before titles existed. */
  title: string;
  folder: string;
  prompt: string;
  mode: Mode;
  reason: string;
  problem: string | null;
  /** `created`: an automation card whose automation was saved. */
  state: "proposed" | "started" | "created" | "discarded";
  sessionId: string | null;
  /** A follow-up for this existing session: Send gives it the prompt instead of starting one. */
  target?: string | null;
  /** Started without Run, because its folder runs cards without asking. */
  auto?: boolean;
  /** An automation card: Create saves an automation on this cron schedule instead of starting a session. */
  schedule?: string | null;
  /** The automation Create saved. */
  automationId?: string | null;
}

export interface ChatMessage {
  id: string;
  threadId: string;
  role: "user" | "planner" | "error";
  text: string;
  cards: DispatchCard[];
  createdAt: number;
}

/** Mirrors `db::ChatThread`: one conversation with the planner. */
export interface ChatThread {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
}

export interface Settings {
  onboarded: boolean;
  chatCli: CliKind | null;
  notifyWaiting: boolean;
  notifyDone: boolean;
  notifyError: boolean;
  companionEnabled: boolean;
  companionPort: number;
  cliPaths: Record<string, string>;
  cliArgs: Record<string, string>;
  scanSeconds: number;
  plannerCanRead: boolean;
  projectRoots: string[];
  permissionMode: PermMode;
  /** Percent, one of `TEXT_SIZES` in format.ts. */
  textSize: number;
  /** The chat planner's model and thinking level, keyed by CLI. Missing means the CLI's own default. */
  chatModels: Record<string, ChatModel>;
  /** What answers in Chat: the chosen CLI, or the model in `plannerApi`. */
  plannerSource: "cli" | "api";
  plannerApi: PlannerApi;
  /** Days a finished session is kept before it is deleted with its logs; 0 keeps it. */
  keepDays: number;
  /** Notifications per CLI, keyed by CLI. A CLI without a rule sends all. */
  notifyClis: Record<string, NotifyRule>;
  /** Notifications per project folder, keyed by path; folders inside it follow it too. */
  notifyProjects: Record<string, NotifyRule>;
  /** Closing the window keeps the app in the tray with its sessions running. */
  closeToTray: boolean;
  /** The one-time "still running in the tray" notice was shown. */
  trayHintShown: boolean;
  /** Start at sign-in, into the tray (Windows); read from Windows each time. */
  startAtLogin: boolean;
  /** Folders whose Chat cards start without Run, with the folders inside them. */
  autoRunFolders: string[];
  /** UI language; the phone follows it too. */
  language: "en" | "id";
}

/** Mirrors `db::NotifyRule`: which notifications one CLI or one folder sends. */
export interface NotifyRule {
  waiting: boolean;
  done: boolean;
  error: boolean;
}

/** Mirrors `db::PlannerApi`: an OpenAI-compatible chat completions endpoint. */
export interface PlannerApi {
  /** The address before `/chat/completions`. */
  baseUrl: string;
  model: string;
  /** Empty for local servers that need none. */
  apiKey: string;
}

/** Mirrors `db::ChatModel`. Empty strings keep the CLI's own default. */
export interface ChatModel {
  model: string;
  effort: string;
}

/** Mirrors `models::ModelOption`. */
export interface ModelOption {
  id: string;
  label: string;
  /** Provider, for a CLI that reaches several (OpenCode). */
  group: string | null;
  /** Thinking levels, least first; empty when the model has none. */
  efforts: string[];
}

/** Mirrors `models::ModelList`. */
export interface ModelList {
  models: ModelOption[];
  /** Thinking levels that are safe with the CLI's own default model. */
  defaultEfforts: string[];
}

/** Mirrors `projects::ProjectFolder`. */
export interface ProjectFolder {
  path: string;
  name: string;
  markers: string[];
  source: "recent" | "open" | "claude" | "root";
}

export interface Project {
  path: string;
  lastUsed: number;
}

export interface CompanionStatus {
  running: boolean;
  address: string | null;
  port: number;
  error: string | null;
}

export interface Pairing {
  code: string;
  url: string;
  expiresAt: number;
}

export interface Device {
  id: string;
  name: string;
  createdAt: number;
  lastSeen: number | null;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  counts: Record<string, number>;
  /** Settings can offer the start at sign-in (Windows only for now). */
  canStartAtLogin: boolean;
}

export interface StartRequest {
  cli: CliKind;
  cwd: string;
  mode: Mode;
  prompt: string;
  permissionMode?: string | null;
  source?: string;
  cols?: number;
  rows?: number;
}

/** Mirrors `db::Automation`: a session started by itself on a cron schedule, in local time. */
export interface Automation {
  id: string;
  name: string;
  cli: CliKind;
  cwd: string;
  mode: Mode;
  prompt: string;
  permissionMode: PermMode;
  /** Five cron fields: minute, hour, day of month, month, day of week. */
  schedule: string;
  enabled: boolean;
  /** `null` while paused. */
  nextRunAt: number | null;
  createdAt: number;
  updatedAt: number;
}

export type RunOutcome = "started" | "late" | "skipped" | "failed";

/** Mirrors `db::AutomationRun`. `sessionId` is null when nothing started or the session was deleted. */
export interface AutomationRun {
  id: string;
  automationId: string;
  dueAt: number;
  ranAt: number;
  outcome: RunOutcome;
  sessionId: string | null;
  error: string | null;
}

/** Mirrors `automations::View`: a list row. */
export interface AutomationView extends Automation {
  lastRun: AutomationRun | null;
}

/** Mirrors `automations::Detail`. */
export interface AutomationDetail {
  automation: Automation;
  runs: AutomationRun[];
}

/** Mirrors `automations::Draft`: what the form saves. */
export interface AutomationDraft {
  name: string;
  cli: CliKind;
  cwd: string;
  mode: Mode;
  prompt: string;
  permissionMode: PermMode;
  schedule: string;
  enabled: boolean;
}

/** Mirrors `terminal::Shell`: a shell found on this computer. */
export interface ShellInfo {
  id: string;
  label: string;
  path: string;
}

/** Mirrors `terminal::TerminalInfo`: a plain shell in a session's folder, not an AI session. */
export interface TerminalInfo {
  id: string;
  /** The session whose screen shows its tab. */
  sessionId: string;
  cwd: string;
  shell: string;
  shellLabel: string;
  pid: number | null;
  running: boolean;
  exitCode: number | null;
  startedAt: number;
}

export type Load<T> =
  | { state: "loading" }
  | { state: "ready"; data: T }
  | { state: "error"; message: string };

/** Mirrors `files::MEDIA_TOO_LARGE`. */
export const MEDIA_TOO_LARGE = "This file is larger than 50 MB.";

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const api = {
  detectClis: () => invoke<CliInstall[]>("detect_clis"),
  scanExternal: () => invoke<ExternalSession[]>("scan_external"),
  outsideDetail: (pid: number) => invoke<{ session: ExternalSession; transcript: Transcript }>("outside_detail", { pid }),
  scanSkills: () => invoke<SkillScan>("scan_skills"),
  listSessions: (limit?: number) => invoke<SessionView[]>("list_sessions", { limit }),
  searchSessions: (query: SessionQuery) => invoke<SessionView[]>("search_sessions", { query }),
  /** Folders sessions ran in, most recent first. */
  sessionFolders: () => invoke<string[]>("session_folders"),
  getSession: (id: string) => invoke<SessionDetail>("get_session", { id }),
  /** The terminal text so far; `seq` is its last chunk, so live chunks up to it are already in it. */
  sessionOutput: (id: string) => invoke<{ data: string; seq: number }>("session_output", { id }),
  /** Null when the session is not running. */
  sessionUsage: (id: string) => invoke<Usage | null>("session_usage", { id }),
  /** The AI CLIs running in each session's terminals, its own and its shell tabs. Sessions with none are left out. */
  sessionClis: () => invoke<Record<string, CliKind[]>>("session_clis"),
  startSession: (req: StartRequest) => invoke<SessionInfo>("start_session", { req }),
  sendInput: (id: string, text: string) => invoke<SessionInfo>("send_input", { id, text }),
  resizeSession: (id: string, cols: number, rows: number) => invoke<void>("resize_session", { id, cols, rows }),
  /** The session page opened (`on`) or closed, so a session on screen does not notify that it finished. */
  setViewing: (id: string, on: boolean) => invoke<void>("set_viewing", { id, on }),
  stopSession: (id: string) => invoke<void>("stop_session", { id }),
  /** Closes the CLI like Stop, but the session ends Done. */
  markSessionDone: (id: string) => invoke<void>("mark_session_done", { id }),
  answerSession: (id: string, allow: boolean) => invoke<SessionInfo>("answer_session", { id, allow }),
  resumeSession: (id: string, cols?: number, rows?: number) => invoke<SessionInfo>("resume_session", { id, cols, rows }),
  deleteHistory: () => invoke<void>("delete_history"),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  recentProjects: () => invoke<Project[]>("recent_projects"),
  projectFolders: () => invoke<ProjectFolder[]>("project_folders"),
  defaultProjectRoots: () => invoke<string[]>("default_project_roots"),
  folderExists: (path: string) => invoke<boolean>("folder_exists", { path }),

  /** One folder of a session's folder; `dir` "" is its top. */
  folderList: (id: string, dir: string) => invoke<FolderListing>("folder_list", { id, dir }),
  folderFind: (id: string, query: string, contents: boolean) => invoke<FindResult>("folder_find", { id, query, contents }),
  folderRead: (id: string, path: string) => invoke<FileText>("folder_read", { id, path }),
  /** An image or a PDF as bytes. A file over 50 MB is refused with `MEDIA_TOO_LARGE`. */
  folderReadBytes: (id: string, path: string) => invoke<ArrayBuffer>("folder_read_bytes", { id, path }),
  gitStatus: (id: string) => invoke<GitStatus>("git_status", { id }),
  gitDiff: (id: string, change: Pick<GitChange, "path" | "oldPath" | "code">) =>
    invoke<GitDiff>("git_diff", { id, path: change.path, oldPath: change.oldPath, untracked: change.code === "?" }),
  gitBranches: (id: string) => invoke<GitBranches>("git_branches", { id }),
  /** Refused while a session in the folder is live or tracked files have uncommitted changes. */
  gitSwitch: (id: string, branch: string) => invoke<GitStatus>("git_switch", { id, branch }),

  /** Installed shells, the default first. */
  terminalShells: () => invoke<ShellInfo[]>("terminal_shells"),
  /** A session's open terminals in tab order. */
  terminalList: (sessionId: string) => invoke<TerminalInfo[]>("terminal_list", { sessionId }),
  /** Opens a shell in the session's folder; `shell` null starts the default one. */
  terminalOpen: (sessionId: string, shell: string | null, cols?: number, rows?: number) =>
    invoke<TerminalInfo>("terminal_open", { sessionId, shell, cols, rows }),
  terminalWrite: (id: string, data: string) => invoke<void>("terminal_write", { id, data }),
  terminalResize: (id: string, cols: number, rows: number) => invoke<void>("terminal_resize", { id, cols, rows }),
  /** Like `sessionOutput`, for a terminal. */
  terminalOutput: (id: string) => invoke<{ data: string; seq: number }>("terminal_output", { id }),
  terminalRestart: (id: string) => invoke<TerminalInfo>("terminal_restart", { id }),
  /** Forgets a terminal's output so far, so it opens empty next time. */
  terminalClear: (id: string) => invoke<void>("terminal_clear", { id }),
  /** Ends the shell and what it started, such as a dev server. */
  terminalClose: (id: string) => invoke<void>("terminal_close", { id }),
  /** Saves an image pasted into a session or terminal to a temporary file and returns its path. */
  savePastedImage: (image: Uint8Array, type: string) =>
    invoke<string>("save_pasted_image", image, { headers: { "x-image-type": type } }),

  chatThreads: () => invoke<ChatThread[]>("chat_threads"),
  chatHistory: (threadId: string) => invoke<ChatMessage[]>("chat_history", { threadId }),
  /** `threadId` null starts a new chat. */
  chatSend: (threadId: string | null, message: string) =>
    invoke<{ thread: ChatThread; user: ChatMessage; reply: ChatMessage }>("chat_send", { threadId, message }),
  chatDeleteThread: (threadId: string) => invoke<void>("chat_delete_thread", { threadId }),
  /** Threads where the planner is answering now, from this window or from a phone. */
  chatAnswering: () => invoke<string[]>("chat_answering"),
  chatUpdateCard: (messageId: string, card: DispatchCard) => invoke<ChatMessage>("chat_update_card", { messageId, card }),
  chatDiscardCard: (messageId: string, cardId: string, undo = false) =>
    invoke<ChatMessage>("chat_discard_card", { messageId, cardId, undo }),
  chatRunCard: (messageId: string, cardId: string) => invoke<ChatMessage>("chat_run_card", { messageId, cardId }),
  chatModels: (cli: CliKind) => invoke<ModelList>("chat_models", { cli }),
  chatSetModel: (cli: CliKind, model: string, effort: string) => invoke<Settings>("chat_set_model", { cli, model, effort }),

  listAutomations: () => invoke<AutomationView[]>("list_automations"),
  getAutomation: (id: string) => invoke<AutomationDetail>("get_automation", { id }),
  /** `id` null creates one. */
  saveAutomation: (id: string | null, draft: AutomationDraft) => invoke<Automation>("save_automation", { id, draft }),
  setAutomationEnabled: (id: string, enabled: boolean) => invoke<Automation>("set_automation_enabled", { id, enabled }),
  deleteAutomation: (id: string) => invoke<void>("delete_automation", { id }),
  runAutomation: (id: string) => invoke<AutomationRun>("run_automation", { id }),
  /** The next three times the schedule comes round, in ms. */
  previewSchedule: (schedule: string) => invoke<number[]>("preview_schedule", { schedule }),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  companionStatus: () => invoke<CompanionStatus>("companion_status"),
  startPairing: () => invoke<Pairing>("start_pairing"),
  listDevices: () => invoke<Device[]>("list_devices"),
  removeDevice: (id: string) => invoke<void>("remove_device", { id }),
  appInfo: () => invoke<AppInfo>("app_info"),
};

/** True inside the Tauri webview; false in the phone browser. */
export function inDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
