// Git status of one session's folder, shared by the Files, Changes and Branch tabs and the viewer.
import { api, errorText, type GitChange, type GitStatus } from "./api";

export class GitWatch {
  status = $state<GitStatus | null>(null);
  state = $state<"loading" | "ready" | "error">("loading");
  error = $state("");
  /** Goes up whenever the status changes, so views that show files read them again. */
  version = $state(0);
  #seen = "";
  #pending: Promise<void> | null = null;

  constructor(readonly id: string) {}

  /** Reads the status; a call while one runs waits for that one instead of starting another. */
  refresh(): Promise<void> {
    this.#pending ??= this.#read().finally(() => (this.#pending = null));
    return this.#pending;
  }

  async #read() {
    try {
      const next = await api.gitStatus(this.id);
      const seen = JSON.stringify(next);
      if (seen !== this.#seen) {
        this.#seen = seen;
        this.status = next;
        this.version += 1;
      }
      this.state = "ready";
      this.error = "";
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }

  change(path: string): GitChange | undefined {
    return this.status?.changes.find((c) => c.path === path);
  }
}

/** What the viewer above the terminal shows: a file's text (at a line, from a search) or its diff. */
export type ViewTarget = { kind: "file" | "diff"; path: string; line?: number };

/** A file's name and the folder it sits in, from a relative path. */
export function splitPath(path: string): { name: string; dir: string } {
  const at = path.lastIndexOf("/");
  return at < 0 ? { name: path, dir: "" } : { name: path.slice(at + 1), dir: path.slice(0, at) };
}
