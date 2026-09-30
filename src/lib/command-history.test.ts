import { afterEach, expect, test, vi } from "vitest";
import { backend } from "../test/fixtures";
import { forgetSaved, loadHistory, remember, resetHistory, suggestFor } from "./command-history";

afterEach(resetHistory);

function load(folder: string, history: { here?: string[]; elsewhere?: string[]; imported?: string[] }) {
  backend({ shell_history: () => ({ here: [], elsewhere: [], imported: [], ...history }) });
  loadHistory(folder);
}

test("a folder's history is read once, and suggestions wait for it", async () => {
  load("/app", { here: ["bun run dev"], imported: ["bun install"] });
  expect(suggestFor("/app", "bun")).toBeNull();
  await vi.waitFor(() => expect(suggestFor("/app", "bun")).toBe("bun run dev"));
  expect(suggestFor("/app", "bun i")).toBe("bun install");
  expect(suggestFor("/web", "bun")).toBeNull();
});

test("a command that just ran comes first in its folder and among other folders' commands elsewhere", async () => {
  load("/app", { here: ["git status"] });
  load("/web", { here: ["git stash"], elsewhere: ["git status"] });
  await vi.waitFor(() => expect(suggestFor("/web", "git")).toBe("git stash"));
  remember("/app", "git switch main");
  expect(suggestFor("/app", "git s")).toBe("git switch main");
  // /web still prefers its own commands.
  expect(suggestFor("/web", "git s")).toBe("git stash");
  expect(suggestFor("/web", "git sw")).toBe("git switch main");
  remember("/web", "git status");
  expect(suggestFor("/web", "git st")).toBe("git status");
});

test("a command heard while the folder's history is being read is not lost", async () => {
  let answer: (h: unknown) => void = () => undefined;
  backend({ shell_history: () => new Promise((r) => (answer = r)) });
  loadHistory("/app");
  remember("/app", "cargo test");
  answer({ here: ["cargo build"], elsewhere: [], imported: [] });
  await vi.waitFor(() => expect(suggestFor("/app", "cargo")).toBe("cargo test"));
  expect(suggestFor("/app", "cargo b")).toBe("cargo build");
});

test("after Delete saved commands only the shells' own history is suggested", async () => {
  load("/app", { here: ["npm test"], imported: ["npm ci"] });
  await vi.waitFor(() => expect(suggestFor("/app", "npm")).toBe("npm test"));
  forgetSaved();
  expect(suggestFor("/app", "npm")).toBe("npm ci");
});
