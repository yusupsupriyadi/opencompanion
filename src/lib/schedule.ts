// Automation schedules. The backend stores five cron fields in the computer's local time; the form's
// simple modes (every day, some days, every N minutes or hours) are written as cron here and read
// back from it, so a schedule saved on the phone opens in the same mode on the desktop.
import { i18n, t, type Key } from "./i18n.svelte";

export type Unit = "minutes" | "hours";
export type Simple =
  | { kind: "daily"; time: string }
  | { kind: "days"; days: number[]; time: string }
  | { kind: "every"; n: number; unit: Unit };
export type ScheduleKind = Simple["kind"] | "cron";

/** Steps that divide an hour or a day evenly, so every run lands on the same clock times. */
export const MINUTE_STEPS = [5, 10, 15, 20, 30];
export const HOUR_STEPS = [1, 2, 3, 4, 6, 8, 12];
/** Monday first, as the week is read in both languages. Cron numbers Sunday 0. */
export const WEEK = [1, 2, 3, 4, 5, 6, 0];
const WEEKDAYS = "1,2,3,4,5";

const pad = (n: number) => String(n).padStart(2, "0");

function hm(time: string): [number, number] {
  const [h, m] = time.split(":").map(Number);
  return [h || 0, m || 0];
}

/** `""` for Some days with no day picked: there is no schedule to save. */
export function toCron(s: Simple): string {
  if (s.kind === "every") {
    if (s.unit === "minutes") return `*/${s.n} * * * *`;
    return s.n === 1 ? "0 * * * *" : `0 */${s.n} * * *`;
  }
  const [h, m] = hm(s.time);
  if (s.kind === "daily") return `${m} ${h} * * *`;
  const days = [...new Set(s.days)].sort((a, b) => a - b);
  if (days.length === 0) return "";
  if (days.length === 7) return `${m} ${h} * * *`;
  const list = days.join(",");
  return `${m} ${h} * * ${list === WEEKDAYS ? "1-5" : list}`;
}

/** Days of the week from `1-5`, `1,3` or `6-7`; `null` for anything else, such as names. */
function readDays(field: string): number[] | null {
  const days = new Set<number>();
  for (const part of field.split(",")) {
    const r = part.match(/^([0-7])(?:-([0-7]))?$/);
    if (!r) return null;
    const from = Number(r[1]);
    const to = r[2] ? Number(r[2]) : from;
    if (to < from) return null;
    for (let d = from; d <= to; d++) days.add(d % 7);
  }
  return [...days].sort((a, b) => a - b);
}

/** The simple mode a cron expression is, or `null` when only Cron can show it. */
export function fromCron(expr: string): Simple | null {
  const f = expr.trim().split(/\s+/);
  if (f.length !== 5) return null;
  const [min, hour, dom, mon, dow] = f;
  if (dom !== "*" || mon !== "*") return null;
  const step = /^\*\/(\d+)$/;
  if (hour === "*" && dow === "*") {
    const n = Number(min.match(step)?.[1]);
    if (MINUTE_STEPS.includes(n)) return { kind: "every", n, unit: "minutes" };
  }
  if (min === "0" && dow === "*") {
    if (hour === "*") return { kind: "every", n: 1, unit: "hours" };
    const n = Number(hour.match(step)?.[1]);
    if (HOUR_STEPS.includes(n)) return { kind: "every", n, unit: "hours" };
  }
  if (!/^\d{1,2}$/.test(min) || !/^\d{1,2}$/.test(hour) || Number(min) > 59 || Number(hour) > 23) return null;
  const time = `${pad(Number(hour))}:${pad(Number(min))}`;
  if (dow === "*") return { kind: "daily", time };
  const days = readDays(dow);
  if (!days) return null;
  return days.length === 7 ? { kind: "daily", time } : { kind: "days", days, time };
}

export function dayShort(d: number): string {
  return t(`auto.day.${d}` as Key);
}

export function dayLong(d: number): string {
  return t(`auto.dayLong.${d}` as Key);
}

/** "Weekdays at 09:00", "Every 15 minutes", or the cron itself when no words fit. */
export function describe(expr: string): string {
  const s = fromCron(expr);
  if (!s) return t("auto.sched.cronWords", { expr: expr.trim() });
  if (s.kind === "every") {
    if (s.unit === "minutes") return t("auto.sched.everyMinutes", { n: s.n });
    return s.n === 1 ? t("auto.sched.everyHour") : t("auto.sched.everyHours", { n: s.n });
  }
  const time = s.time;
  if (s.kind === "daily") return t("auto.sched.dailyAt", { time });
  const list = s.days.join(",");
  if (list === WEEKDAYS) return t("auto.sched.weekdaysAt", { time });
  if (list === "0,6") return t("auto.sched.weekendsAt", { time });
  const days = WEEK.filter((d) => s.days.includes(d)).map(dayShort).join(", ");
  return t("auto.sched.daysAt", { days, time });
}

function dayStart(at: number): number {
  const d = new Date(at);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "today 09:00", "tomorrow 09:00", else "Mon 5 Oct 09:00". Times are 24-hour, as cron writes them. */
export function whenText(at: number, now = Date.now()): string {
  const d = new Date(at);
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  const days = Math.round((dayStart(at) - dayStart(now)) / 86_400_000);
  if (days === 0) return t("auto.when.today", { time });
  if (days === 1) return t("auto.when.tomorrow", { time });
  // en-US writes "Sep", as the backend titles a run's session; en-GB would write "Sept".
  const month = d.toLocaleDateString(i18n.lang === "id" ? "id-ID" : "en-US", { month: "short" }).replace(".", "");
  return `${dayShort(d.getDay())} ${d.getDate()} ${month} ${time}`;
}
