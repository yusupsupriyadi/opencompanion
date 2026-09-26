import { afterEach, expect, test } from "vitest";
import { session } from "../test/fixtures";
import { trackLine } from "./format";
import { i18n, plural, t, tb } from "./i18n.svelte";

afterEach(() => {
  i18n.lang = "en";
});

test("placeholders take the text as it is, even with $ patterns or braces in it", () => {
  expect(t("chat.card.followUpFor", { title: "Price $& tax $$ done" })).toBe("Follow-up for Price $& tax $$ done");
  expect(t("chat.card.followUpFor", { title: "Rename {title} and {cli}" })).toBe("Follow-up for Rename {title} and {cli}");
});

test("a session row's last event from the backend reads in the chosen language", () => {
  const s = session({ status: "error", lastEvent: "Exited with code 1", endedAt: Date.now() });
  expect(trackLine(s)).toMatch(/^Exited with code 1 · /);
  i18n.lang = "id";
  expect(trackLine(s)).toMatch(/^Keluar dengan kode 1 · /);
});

test("the language switch changes t, plural and backend messages together", () => {
  expect(plural(2, "chat.announce.cardsOne", "chat.announce.cardsMany")).toBe("The planner answered with 2 session cards.");
  expect(tb("This folder does not exist.")).toBe("This folder does not exist.");
  i18n.lang = "id";
  expect(t("chat.card.followUpFor", { title: "Tes login" })).toBe("Pesan lanjutan untuk Tes login");
  expect(plural(1, "chat.announce.cardsOne", "chat.announce.cardsMany")).toBe("Planner menjawab dengan 1 kartu sesi.");
  expect(tb("This folder does not exist.")).toBe("Folder ini tidak ada.");
});
