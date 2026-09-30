import AppWindow from "phosphor-svelte/lib/AppWindow";
import Bell from "phosphor-svelte/lib/Bell";
import Binoculars from "phosphor-svelte/lib/Binoculars";
import Books from "phosphor-svelte/lib/Books";
import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
import CircleHalf from "phosphor-svelte/lib/CircleHalf";
import ClockCounterClockwise from "phosphor-svelte/lib/ClockCounterClockwise";
import DeviceMobile from "phosphor-svelte/lib/DeviceMobile";
import FolderSimple from "phosphor-svelte/lib/FolderSimple";
import ShieldCheck from "phosphor-svelte/lib/ShieldCheck";
import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
import TextAa from "phosphor-svelte/lib/TextAa";
import Translate from "phosphor-svelte/lib/Translate";
import type { Component } from "svelte";
import { t, type Key } from "./i18n.svelte";

// The Settings sidebar. Every item is its own screen: CLIs and Skills are routes of their own, the
// rest are sections of /settings picked with ?s=. Phone access is what a bare /settings shows.
export type SectionId =
  | "phone"
  | "window"
  | "language"
  | "planner"
  | "permissions"
  | "projects"
  | "notifications"
  | "outside"
  | "history"
  | "theme"
  | "text-size";

export type SettingsItem = {
  id: SectionId | "clis" | "skills";
  href: string;
  key: Key;
  icon: Component;
  // Search also looks in these strings, so "tray" finds Window and sign-in.
  terms: Key[];
};

const section = (id: SectionId, key: Key, icon: Component, terms: Key[]): SettingsItem => ({
  id,
  href: id === "phone" ? "/settings" : `/settings?s=${id}`,
  key,
  icon,
  terms,
});

export const SETTINGS_GROUPS: { key: Key; items: SettingsItem[] }[] = [
  {
    key: "settings.group.general",
    items: [
      section("phone", "settings.phone.title", DeviceMobile, ["settings.phone.desc"]),
      section("window", "settings.window.title", AppWindow, ["settings.window.tray", "settings.window.login"]),
      section("language", "settings.language.title", Translate, ["settings.language.label"]),
    ],
  },
  {
    key: "settings.group.agents",
    items: [
      { id: "clis", href: "/settings/clis", key: "work.clis.heading", icon: TerminalWindow, terms: ["work.clis.sub"] },
      { id: "skills", href: "/settings/skills", key: "work.skills.heading", icon: Books, terms: ["work.skills.sub"] },
      section("planner", "settings.planner.title", ChatsCircle, ["settings.planner.label", "settings.autoRun.title"]),
      section("permissions", "settings.perm.title", ShieldCheck, ["settings.perm.desc"]),
    ],
  },
  {
    key: "settings.group.workspace",
    items: [
      section("projects", "settings.projects.title", FolderSimple, ["settings.projects.desc"]),
      section("notifications", "settings.notify.title", Bell, ["settings.notify.waiting", "settings.notify.done", "settings.notify.error"]),
      section("outside", "settings.scan.title", Binoculars, ["settings.scan.label"]),
      section("history", "settings.history.title", ClockCounterClockwise, ["settings.history.keepLabel", "settings.history.delete", "settings.commands.suggest"]),
    ],
  },
  {
    key: "settings.group.appearance",
    items: [
      section("theme", "settings.theme.title", CircleHalf, ["settings.theme.desc", "settings.theme.day", "settings.theme.dusk"]),
      section("text-size", "settings.text.title", TextAa, ["settings.text.desc"]),
    ],
  },
];

const ITEMS = SETTINGS_GROUPS.flatMap((g) => g.items);
const ROUTES = ITEMS.filter((i) => i.id === "clis" || i.id === "skills");
const SECTIONS = ITEMS.filter((i) => !ROUTES.includes(i));

/** The section of /settings that `url` asks for; anything unknown shows Phone access. */
export function sectionOf(url: URL): SectionId {
  const s = url.searchParams.get("s");
  return (SECTIONS.find((i) => i.id === s)?.id ?? "phone") as SectionId;
}

/** The sidebar item for the screen `url` shows. */
export function currentItem(url: URL): SettingsItem {
  const path = url.pathname.replace(/\/+$/, "");
  const id = sectionOf(url);
  return ROUTES.find((i) => i.href === path) ?? SECTIONS.find((i) => i.id === id) ?? SECTIONS[0];
}

/** The groups with only the items whose name or text holds `query`, in the current language. */
export function filterGroups(query: string) {
  const q = query.trim().toLocaleLowerCase();
  if (!q) return SETTINGS_GROUPS;
  const holds = (k: Key) => t(k).toLocaleLowerCase().includes(q);
  return SETTINGS_GROUPS.map((g) => ({ ...g, items: g.items.filter((i) => holds(i.key) || i.terms.some(holds)) })).filter((g) => g.items.length);
}
