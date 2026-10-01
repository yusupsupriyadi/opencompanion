import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type UpdateView } from "./api";
import { isLive } from "./format";
import { t } from "./i18n.svelte";
import { app } from "./store.svelte";

// What the backend updater reports (`updater.rs`), for the sidebar card and Settings › Updates.
// `confirming` opens the dialog the shell renders once, before a restart that stops sessions.
export const update = $state({ view: null as UpdateView | null, confirming: false });

let started = false;

/** Called once by the desktop shell. */
export async function startUpdates() {
  if (started) return;
  started = true;
  await listen<UpdateView>("update-changed", (e) => (update.view = e.payload));
  try {
    update.view = await api.updateStatus();
  } catch {
    // A build without the updater: the card and the Settings section stay hidden.
  }
}

/** Sessions the restart would stop: their CLI, or the shell a terminal runs in. */
export function liveSessions(): number {
  return app.sessions.filter(isLive).length;
}

/** Update asks first when the restart would stop running sessions. */
export function askInstall() {
  if (liveSessions() > 0) update.confirming = true;
  else install();
}

export async function install() {
  update.confirming = false;
  try {
    await api.installUpdate();
  } catch {
    // The failure comes back in `update-changed`, where the card and Settings show it.
  }
}

/** The release page of `version`, whose notes say what changed. */
export function openRelease(version: string) {
  openUrl(`https://github.com/yusupsupriyadi/opencompanion/releases/tag/v${version}`).catch(() => undefined);
}

/** "45%", or the megabytes so far when the server sends no size. */
export function progressText(v: UpdateView): string {
  if (v.total) return t("shell.update.percent", { n: Math.min(100, Math.floor((v.received / v.total) * 100)) });
  return t("shell.update.megabytes", { n: Math.round(v.received / 1048576) });
}
