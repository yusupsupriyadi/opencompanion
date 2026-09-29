import { goto } from "$app/navigation";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import Copy from "phosphor-svelte/lib/Copy";
import FolderOpen from "phosphor-svelte/lib/FolderOpen";
import Plus from "phosphor-svelte/lib/Plus";
import PushPin from "phosphor-svelte/lib/PushPin";
import PushPinSlash from "phosphor-svelte/lib/PushPinSlash";
import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
import Trash from "phosphor-svelte/lib/Trash";
import { errorText, type SessionView } from "./api";
import { folderName, runsCli } from "./format";
import { t } from "./i18n.svelte";
import { currentPlatform } from "./platform";
import { askDelete, askNewSession, showToast } from "./store.svelte";

export type MenuItem = {
  label: string;
  icon?: typeof Trash;
  action: () => void;
  danger?: boolean;
  /** Listed but refused; `hint` says why. */
  disabled?: boolean;
  hint?: string;
};
/** `null` draws a divider. */
export type MenuEntry = MenuItem | null;

/** The one right-click menu, drawn by the shell. `key` names the row it is for, so that row stays lit. */
export const menu = $state({ open: false, seq: 0, key: "", label: "", x: 0, y: 0, items: [] as MenuEntry[] });
let opener: Element | null = null;

/** Opens the menu for the row that handles `e`, in place of the webview's own menu. */
export function openMenu(e: MouseEvent, key: string, label: string, items: MenuEntry[]) {
  e.preventDefault();
  const row = (e.currentTarget as HTMLElement).getBoundingClientRect();
  // Shift+F10 and the Menu key fire on the focused row, and the webview then gives a point
  // that can sit outside it: the menu opens under the row instead.
  const inRow = e.clientX >= row.left && e.clientX <= row.right && e.clientY >= row.top && e.clientY <= row.bottom;
  opener = document.activeElement;
  Object.assign(menu, {
    open: true,
    seq: menu.seq + 1,
    key,
    label,
    items,
    x: inRow ? e.clientX : row.left + 8,
    y: inRow ? e.clientY : row.bottom,
  });
}

/** `restore` puts focus back where it was before the menu opened. */
export function closeMenu(restore = true) {
  if (!menu.open) return;
  menu.open = false;
  menu.key = "";
  if (restore && opener instanceof HTMLElement && opener.isConnected) opener.focus();
  opener = null;
}

const REVEAL = { windows: "shell.menu.revealWindows", macos: "shell.menu.revealMac", linux: "shell.menu.revealLinux" } as const;

/** Every row about a folder offers these two. */
export function folderEntries(cwd: string): MenuItem[] {
  return [
    { label: t("shell.menu.copyPath"), icon: Copy, action: () => copyPath(cwd) },
    { label: t(REVEAL[currentPlatform()]), icon: FolderOpen, action: () => reveal(cwd) },
  ];
}

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    showToast(t("shell.menu.copied", { path }));
  } catch {
    showToast(t("shell.menu.copyFailed"));
  }
}

async function reveal(path: string) {
  try {
    await revealItemInDir(path);
  } catch (e) {
    showToast(t("shell.menu.revealFailed", { folder: folderName(path), error: errorText(e) }));
  }
}

/** A session row's menu. The sidebar passes `pin`; lists without pins leave it out. */
export function sessionMenu(s: SessionView, pin?: { pinned: boolean; toggle: () => void }): MenuEntry[] {
  const running = runsCli(s);
  // Headless sessions have Stop; a terminal's CLI is exited in the terminal.
  const hint = s.mode === "headless" ? t("shell.menu.stopFirst") : t("shell.menu.exitFirst");
  return [
    { label: t("shell.menu.open"), icon: TerminalWindow, action: () => goto(`/session?id=${encodeURIComponent(s.id)}`) },
    { label: t("shell.sidebar.newSessionIn", { folder: folderName(s.cwd) }), icon: Plus, action: () => askNewSession(s.cwd) },
    ...(pin
      ? [{ label: t(pin.pinned ? "shell.sidebar.unpinSession" : "shell.sidebar.pinSession"), icon: pin.pinned ? PushPinSlash : PushPin, action: pin.toggle }]
      : []),
    null,
    ...folderEntries(s.cwd),
    null,
    // The backend refuses to delete a session while a CLI runs in it, so the item says so up
    // front. A terminal at its shell prompt closes with the session.
    { label: t("shell.deleteSession"), icon: Trash, danger: true, disabled: running, hint: running ? hint : undefined, action: () => askDelete(s) },
  ];
}
