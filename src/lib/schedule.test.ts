import { afterEach, expect, test } from "vitest";
import { i18n } from "./i18n.svelte";
import { describe as words, fromCron, toCron, whenText, type Simple } from "./schedule";

afterEach(() => {
  i18n.lang = "en";
});

test("the simple modes are written as cron and read back the same", () => {
  const cases: [Simple, string][] = [
    [{ kind: "daily", time: "09:30" }, "30 9 * * *"],
    [{ kind: "days", days: [1, 2, 3, 4, 5], time: "09:00" }, "0 9 * * 1-5"],
    [{ kind: "days", days: [3, 1], time: "18:05" }, "5 18 * * 1,3"],
    [{ kind: "every", n: 15, unit: "minutes" }, "*/15 * * * *"],
    [{ kind: "every", n: 2, unit: "hours" }, "0 */2 * * *"],
    [{ kind: "every", n: 1, unit: "hours" }, "0 * * * *"],
  ];
  for (const [simple, cron] of cases) {
    expect(toCron(simple)).toBe(cron);
    const back = fromCron(cron);
    expect(back).toEqual(simple.kind === "days" ? { ...simple, days: [...simple.days].sort() } : simple);
  }
});

test("all seven days is every day, and no day is no schedule", () => {
  expect(toCron({ kind: "days", days: [0, 1, 2, 3, 4, 5, 6], time: "07:00" })).toBe("0 7 * * *");
  expect(toCron({ kind: "days", days: [], time: "07:00" })).toBe("");
});

test("cron the simple modes cannot show stays cron", () => {
  expect(fromCron("0 9 1 * *")).toBeNull();
  expect(fromCron("*/7 * * * *")).toBeNull();
  expect(fromCron("0 */5 * * *")).toBeNull();
  expect(fromCron("0 9 * * MON")).toBeNull();
  expect(fromCron("0 0 9 * * *")).toBeNull();
  // Sunday is 0 or 7.
  expect(fromCron("0 9 * * 6-7")).toEqual({ kind: "days", days: [0, 6], time: "09:00" });
});

test("a schedule reads as words", () => {
  expect(words("30 9 * * *")).toBe("Every day at 09:30");
  expect(words("0 9 * * 1-5")).toBe("Weekdays at 09:00");
  expect(words("0 10 * * 0,6")).toBe("Weekends at 10:00");
  expect(words("0 9 * * 1,3")).toBe("Mon, Wed at 09:00");
  expect(words("0 9 * * 0,1")).toBe("Mon, Sun at 09:00");
  expect(words("*/15 * * * *")).toBe("Every 15 minutes");
  expect(words("0 * * * *")).toBe("Every hour");
  expect(words("0 */2 * * *")).toBe("Every 2 hours");
  expect(words("0 9 1 * *")).toBe("Cron: 0 9 1 * *");
  i18n.lang = "id";
  expect(words("0 9 * * 1-5")).toBe("Hari kerja pukul 09:00");
  expect(words("0 9 * * 1,3")).toBe("Sen, Rab pukul 09:00");
});

test("a run time says today or tomorrow when it is close", () => {
  const now = new Date(2026, 8, 30, 8, 0).getTime();
  expect(whenText(new Date(2026, 8, 30, 9, 0).getTime(), now)).toBe("today 09:00");
  expect(whenText(new Date(2026, 9, 1, 9, 0).getTime(), now)).toBe("tomorrow 09:00");
  expect(whenText(new Date(2026, 9, 5, 9, 0).getTime(), now)).toBe("Mon 5 Oct 09:00");
  expect(whenText(new Date(2026, 8, 27, 2, 19).getTime(), now)).toBe("Sun 27 Sep 02:19");
  i18n.lang = "id";
  expect(whenText(new Date(2026, 9, 5, 9, 0).getTime(), now)).toBe("Sen 5 Okt 09:00");
});
