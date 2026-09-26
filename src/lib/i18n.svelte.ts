// UI language, English or Indonesian (PRD FR-63). Each screen area keeps its own strings in
// `./i18n/<area>.ts`; the Indonesian dictionary is typed against the English one, so a missing
// translation is a type error, not a blank label.
import * as chat from "./i18n/chat";
import * as phone from "./i18n/phone";
import * as sessions from "./i18n/sessions";
import * as settings from "./i18n/settings";
import * as shell from "./i18n/shell";
import * as work from "./i18n/work";

export type Lang = "en" | "id";

const en = { ...shell.en, ...sessions.en, ...chat.en, ...work.en, ...settings.en, ...phone.en };
export type Key = keyof typeof en;
const id: Record<Key, string> = { ...shell.id, ...sessions.id, ...chat.id, ...work.id, ...settings.id, ...phone.id };
const DICTS: Record<Lang, Record<Key, string>> = { en, id };

export const LANGS: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "id", label: "Bahasa Indonesia" },
];

export const i18n = $state({ lang: "en" as Lang });

export function setLang(lang: string | null | undefined) {
  i18n.lang = lang === "id" ? "id" : "en";
  if (typeof document !== "undefined") document.documentElement.lang = i18n.lang;
}

/** The string for `key` in the current language, with `{name}` placeholders filled from `vars`. */
export function t(key: Key, vars?: Record<string, string | number>): string {
  let s: string = DICTS[i18n.lang][key] ?? en[key];
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

/** `one` for exactly 1, else `many`; both get `{n}`. */
export function plural(n: number, one: Key, many: Key, vars: Record<string, string | number> = {}): string {
  return t(n === 1 ? one : many, { n, ...vars });
}
