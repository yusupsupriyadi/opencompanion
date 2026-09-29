// The plain shells open in one session's folder, shown as tabs beside the session's own terminal.
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type TerminalInfo } from "./api";

export class SessionShells {
  list = $state<TerminalInfo[]>([]);
  state = $state<"loading" | "ready" | "error">("loading");
  error = $state("");

  /** Tab names: shells of the same kind are numbered in the order they were opened. */
  names = $derived.by(() => {
    const seen = new Map<string, number>();
    return new Map(
      this.list.map((x) => {
        const n = (seen.get(x.shellLabel) ?? 0) + 1;
        seen.set(x.shellLabel, n);
        return [x.id, n === 1 ? x.shellLabel : `${x.shellLabel} ${n}`];
      }),
    );
  });

  // Closed here, so a late event cannot bring the tab back.
  #closed = new Set<string>();
  // Events that arrive while the list loads, applied on top of it.
  #early: TerminalInfo[] | null = [];

  constructor(readonly sessionId: string) {}

  /** Listens for changes, then loads the list. Returns the function that stops listening. */
  start(): () => void {
    const un = listen<TerminalInfo>("terminal-changed", (e) => {
      if (this.#early) this.#early.push(e.payload);
      else this.#upsert(e.payload);
    });
    un.then(() => this.load());
    return () => {
      un.then((f) => f());
    };
  }

  async load() {
    this.state = "loading";
    this.#early = [];
    try {
      this.list = await api.terminalList(this.sessionId);
      this.#early.splice(0).forEach((info) => this.#upsert(info));
      this.#early = null;
      this.state = "ready";
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }

  /** `keep` leaves a terminal already listed as it is: an event may have been newer than this copy. */
  #upsert(info: TerminalInfo, keep = false) {
    if (info.sessionId !== this.sessionId || this.#closed.has(info.id)) return;
    const i = this.list.findIndex((x) => x.id === info.id);
    if (i < 0) {
      this.#place?.(info.id);
      this.#place = null;
      this.list.push(info);
    } else if (!keep) this.list[i] = info;
  }

  // Called with a shell opened here just before it is listed, so a split puts it in its tab without a tab of its own
  // showing first. Its "changed" event can arrive before `terminalOpen` answers.
  #place: ((id: string) => void) | null = null;

  async open(shell: string, place?: (id: string) => void): Promise<TerminalInfo> {
    this.#place = place ?? null;
    try {
      const info = await api.terminalOpen(this.sessionId, shell);
      this.#upsert(info, true);
      return info;
    } finally {
      this.#place = null;
    }
  }

  /** Ends the shell and what it started, then drops its tab. */
  async close(id: string) {
    await api.terminalClose(id);
    this.#closed.add(id);
    this.list = this.list.filter((x) => x.id !== id);
  }

  async restart(id: string) {
    this.#upsert(await api.terminalRestart(id));
  }
}
