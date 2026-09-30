// What shell tabs suggest from, read once per folder and kept current from the commands shells report.

import { listen } from "@tauri-apps/api/event";
import { api, type ShellHistory } from "./api";
import { suggestion } from "./command-suggest";

const loaded = new Map<string, ShellHistory>();
const loading = new Set<string>();
let listening = false;
// The last commands heard, numbered, so a folder whose history was still on its way takes them in when it arrives.
const heard: { n: number; folder: string; command: string }[] = [];
let heardCount = 0;

/** Moves `command` to the front of `list`. */
function toFront(list: string[], command: string) {
  const at = list.indexOf(command);
  if (at >= 0) list.splice(at, 1);
  list.unshift(command);
}

function apply(f: string, h: ShellHistory, folder: string, command: string) {
  if (f === folder) {
    toFront(h.here, command);
    h.elsewhere = h.elsewhere.filter((c) => c !== command);
  } else if (!h.here.includes(command)) {
    toFront(h.elsewhere, command);
  }
}

/** A command that ran in a shell of `folder`: it now comes first there, and among other folders' commands elsewhere. */
export function remember(folder: string, command: string) {
  heard.push({ n: ++heardCount, folder, command });
  if (heard.length > 100) heard.shift();
  for (const [f, h] of loaded) apply(f, h, folder, command);
}

/** Starts reading `folder`'s history, once; suggestions come after it arrives. */
export function loadHistory(folder: string) {
  if (!listening) {
    listening = true;
    listen<{ folder: string; command: string }>("terminal-command", (e) => remember(e.payload.folder, e.payload.command)).catch(() => {
      listening = false;
    });
  }
  if (loaded.has(folder) || loading.has(folder)) return;
  loading.add(folder);
  const since = heardCount;
  api
    .shellHistory(folder)
    .then((h) => {
      // Heard while the history was read: applying one the read already holds changes nothing.
      for (const e of heard) if (e.n > since) apply(folder, h, e.folder, e.command);
      loaded.set(folder, h);
    })
    .catch(() => undefined)
    .finally(() => loading.delete(folder));
}

/** The command to suggest in a shell of `folder` for what was typed, or null. */
export function suggestFor(folder: string, typed: string): string | null {
  const h = loaded.get(folder);
  return h ? suggestion(typed, [h.here, h.elsewhere, h.imported]) : null;
}

/** After Delete saved commands: only the shells' own history is left to suggest from. */
export function forgetSaved() {
  for (const h of loaded.values()) {
    h.here = [];
    h.elsewhere = [];
  }
}

/** For tests: forgets everything read so far. */
export function resetHistory() {
  loaded.clear();
  loading.clear();
  heard.length = 0;
}
