// Phone companion client (PRD section F). Runs in the phone's browser, talks to the desktop's
// companion server over the LAN with a device token, never to anything else.
import type { CliInstall, EventRow, PermMode, ProjectFolder, SessionInfo } from "./api";

const TOKEN_KEY = "air-token";

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string | null) {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    // Storage refused (private mode): pairing lasts until the tab closes.
  }
  memoryToken = token;
}

let memoryToken: string | null = null;
const token = () => getToken() ?? memoryToken;

export class PhoneError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}

export const phone = $state({
  /** `offline` means the desktop did not answer at all (PRD FR-56). */
  connection: "connecting" as "connecting" | "online" | "offline",
  unpaired: false,
  sessions: [] as SessionInfo[],
  loaded: false,
  notice: "",
});

export async function call<T>(path: string, init: RequestInit = {}): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      ...init,
      headers: {
        "Content-Type": "application/json",
        ...(token() ? { Authorization: `Bearer ${token()}` } : {}),
        ...(init.headers ?? {}),
      },
    });
  } catch {
    // One failed request while the live connection is up is a blip, not a desktop that went away.
    if (socket?.readyState !== WebSocket.OPEN) phone.connection = "offline";
    throw new PhoneError("Can't reach your desktop.", 0);
  }
  phone.connection = "online";
  const body = await res.json().catch(() => ({}));
  if (res.status === 401 && path !== "/api/pair") {
    phone.unpaired = true;
    setToken(null);
  }
  if (!res.ok) throw new PhoneError(body.error ?? `Request failed (${res.status}).`, res.status);
  return body as T;
}

/** Approve or Deny a permission prompt (PRD FR-54). */
export function answer(id: string, allow: boolean) {
  return call<{ session: SessionInfo }>(`/api/sessions/${id}/answer`, { method: "POST", body: JSON.stringify({ allow }) });
}

export async function loadSessions() {
  const r = await call<{ sessions: SessionInfo[] }>("/api/sessions");
  phone.sessions = r.sessions;
  phone.loaded = true;
}

/** What the New session and Run forms can offer, from `GET /api/options`. */
export interface PhoneOptions {
  clis: CliInstall[];
  folders: ProjectFolder[];
  permissionMode: PermMode;
}

/** `tasks` and `chat` say the Board or a chat thread changed; the screen showing it reloads. */
export type PhoneMessage = {
  type: string;
  session?: SessionInfo;
  event?: EventRow;
  title?: string;
  body?: string;
  threadId?: string;
};

type Listener = (msg: PhoneMessage) => void;
const listeners = new Set<Listener>();
export function onMessage(fn: Listener) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

let socket: WebSocket | null = null;
let retry: ReturnType<typeof setTimeout> | undefined;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

/** Close codes the desktop sends (`companion::CLOSE_REMOVED`, `CLOSE_OFF`). */
const CLOSE_REMOVED = 4401;
const RETRY_FIRST = 3000;
const RETRY_MOST = 10000;
let delay = RETRY_FIRST;

function scheduleRetry() {
  clearTimeout(retry);
  retry = setTimeout(reconnect, delay);
  delay = Math.min(delay * 2, RETRY_MOST);
}

/** One reconnect try. A request goes first: a phone removed on the desktop gets a 401 there and
 * goes back to pairing, instead of retrying a socket the desktop will never accept. */
async function reconnect() {
  if (phone.unpaired || !token()) return;
  try {
    await loadSessions();
    connect();
  } catch {
    if (!phone.unpaired) scheduleRetry();
  }
}

/** A short line in the toast at the bottom of the phone screen. */
export function notify(text: string) {
  phone.notice = text;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (phone.notice = ""), 6000);
}

/** One live update from the desktop: sessions change here, then every screen that listens hears it. */
export function receive(msg: PhoneMessage) {
  const s = msg.session;
  if (msg.type === "session" && s) {
    const i = phone.sessions.findIndex((x) => x.id === s.id);
    if (i >= 0) phone.sessions[i] = s;
    else phone.sessions.unshift(s);
  }
  if (msg.type === "resync") loadSessions().catch(() => undefined);
  if (msg.type === "notify") notify(`${msg.title}. ${msg.body}`);
  listeners.forEach((l) => l(msg));
}

/** Live updates (PRD FR-52). Reconnects every few seconds while the desktop is away. */
export function connect() {
  const t = token();
  if (!t || socket) return;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(`${proto}://${location.host}/api/ws?token=${encodeURIComponent(t)}`);
  socket = ws;
  ws.onopen = () => {
    delay = RETRY_FIRST;
    phone.connection = "online";
    loadSessions().catch(() => undefined);
  };
  ws.onmessage = (e) => {
    let msg;
    try {
      msg = JSON.parse(e.data);
    } catch {
      return;
    }
    receive(msg);
  };
  ws.onclose = (e) => {
    // A socket replaced by a newer one closes late; the newer one is in charge.
    if (socket !== ws) return;
    socket = null;
    if (e.code === CLOSE_REMOVED) {
      phone.unpaired = true;
      setToken(null);
      return;
    }
    if (phone.unpaired || !token()) return;
    phone.connection = "offline";
    scheduleRetry();
  };
}

export function reconnectNow() {
  clearTimeout(retry);
  delay = RETRY_FIRST;
  phone.connection = "connecting";
  const old = socket;
  socket = null;
  old?.close();
  reconnect();
}
