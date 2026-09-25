// Phone companion client (PRD section F). Runs in the phone's browser, talks to the desktop's
// companion server over the LAN with a device token, never to anything else.
import type { EventRow, SessionInfo } from "./api";

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
    phone.connection = "offline";
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

export async function loadSessions() {
  const r = await call<{ sessions: SessionInfo[] }>("/api/sessions");
  phone.sessions = r.sessions;
  phone.loaded = true;
}

type Listener = (msg: { type: string; session?: SessionInfo; event?: EventRow; title?: string; body?: string }) => void;
const listeners = new Set<Listener>();
export function onMessage(fn: Listener) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

let socket: WebSocket | null = null;
let retry: ReturnType<typeof setTimeout> | undefined;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

/** Live updates (PRD FR-52). Reconnects every few seconds while the desktop is away. */
export function connect() {
  const t = token();
  if (!t || socket) return;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(`${proto}://${location.host}/api/ws?token=${encodeURIComponent(t)}`);
  socket = ws;
  ws.onopen = () => {
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
    if (msg.type === "session" && msg.session) {
      const i = phone.sessions.findIndex((s) => s.id === msg.session.id);
      if (i >= 0) phone.sessions[i] = msg.session;
      else phone.sessions.unshift(msg.session);
    }
    if (msg.type === "resync") loadSessions().catch(() => undefined);
    if (msg.type === "notify") {
      phone.notice = `${msg.title}. ${msg.body}`;
      clearTimeout(noticeTimer);
      noticeTimer = setTimeout(() => (phone.notice = ""), 6000);
    }
    listeners.forEach((l) => l(msg));
  };
  ws.onclose = () => {
    socket = null;
    if (phone.unpaired || !token()) return;
    phone.connection = "offline";
    clearTimeout(retry);
    retry = setTimeout(connect, 4000);
  };
}

export function reconnectNow() {
  clearTimeout(retry);
  phone.connection = "connecting";
  socket?.close();
  socket = null;
  loadSessions()
    .then(connect)
    .catch(() => undefined);
}
