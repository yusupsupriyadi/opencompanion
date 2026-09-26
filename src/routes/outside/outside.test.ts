import { render, screen, within } from "@testing-library/svelte";
import { beforeEach, expect, test } from "vitest";
import type { ExternalSession, Transcript } from "$lib/api";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend } from "../../test/fixtures";
import OutsidePage from "./+page.svelte";

const proc: ExternalSession = {
  pid: 37100,
  kind: "claude",
  mode: "interactive",
  cwd: "C:\\Users\\me\\Project\\ai-remote",
  startedAt: Math.floor(Date.now() / 1000) - 600,
  cpuPercent: 0,
  memoryBytes: 500 * 1048576,
};

const transcript: Transcript = {
  source: "C:\\Users\\me\\.claude\\projects\\C--Users-me-Project-ai-remote\\a1.jsonl",
  lines: [
    { speaker: "you", text: "Fix the failing tests", at: Date.now() - 120_000 },
    { speaker: "tool", text: "Bash bun run test", at: Date.now() - 90_000 },
    { speaker: "cli", text: "All 12 tests pass now.", at: Date.now() - 60_000 },
  ],
  note: null,
};

beforeEach(() => {
  app.clis = CLIS;
  app.outside = [];
  app.now = Date.now();
});

test("an outside session shows the transcript from the CLI's own history, read-only", async () => {
  setUrl("/outside?pid=37100");
  const calls = backend({ outside_detail: () => ({ session: proc, transcript }) });
  render(OutsidePage);

  expect(await screen.findByRole("heading", { name: "Claude Code in ai-remote" })).toBeInTheDocument();
  expect(calls.calls("outside_detail")[0]).toEqual({ pid: 37100 });
  const out = screen.getByRole("region", { name: "Transcript of Claude Code in ai-remote" });
  expect(out).toHaveTextContent("› Fix the failing tests");
  expect(out).toHaveTextContent("• Bash bun run test");
  expect(out).toHaveTextContent("All 12 tests pass now.");
  expect(within(out).getByText("This session was opened outside OpenCompanion. Showing its transcript, read-only.")).toBeInTheDocument();
  expect(screen.getByText("Read-only")).toBeInTheDocument();
  expect(screen.getByText("~/.claude/projects/C--Users-me-Project-ai-remote/a1.jsonl")).toBeInTheDocument();
  expect(screen.getByText(/Last entry 1 min ago\./)).toBeInTheDocument();
  expect(screen.getByText("500 MB")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /Stop/ })).not.toBeInTheDocument();
});

test("the periodic scan adds CPU, and a CLI without a readable history says why", async () => {
  setUrl("/outside?pid=51");
  const gemini: ExternalSession = { ...proc, pid: 51, kind: "gemini", cwd: null };
  app.outside = [{ ...gemini, cpuPercent: 4.5 }];
  backend({ outside_detail: () => ({ session: gemini, transcript: { source: null, lines: [], note: "Transcript not available for this CLI." } }) });
  render(OutsidePage);

  expect(await screen.findByRole("heading", { name: "Gemini CLI" })).toBeInTheDocument();
  expect(screen.getByRole("navigation", { name: "Breadcrumb" })).toHaveTextContent("Overview / Folder unknown");
  expect(screen.getByRole("region", { name: "Transcript of Gemini CLI" })).toHaveTextContent("Transcript not available for this CLI.");
  expect(screen.getByText("4.5%")).toBeInTheDocument();
});

test("an ended process says so and leads back to the Overview", async () => {
  setUrl("/outside?pid=4");
  backend({ outside_detail: () => new Error("This process has ended.") });
  render(OutsidePage);

  expect(await screen.findByRole("heading", { name: "This session has ended" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Back to Overview" })).toHaveAttribute("href", "/");
});

test("a read that fails shows the reason and can be tried again", async () => {
  setUrl("/outside?pid=37100");
  let fail = true;
  backend({ outside_detail: () => (fail ? new Error("The process list could not be read.") : { session: proc, transcript }) });
  render(OutsidePage);

  expect(await screen.findByRole("alert")).toHaveTextContent("The process list could not be read.");
  fail = false;
  screen.getByRole("button", { name: "Try again" }).click();
  expect(await screen.findByRole("heading", { name: "Claude Code in ai-remote" })).toBeInTheDocument();
});
