import { fireEvent, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { SessionInfo } from "$lib/api";
import { phone, receive } from "$lib/phone.svelte";
import { setUrl } from "../../../test/app-state.svelte";
import { phoneServer, sent } from "../../../test/phone-server";
import Session from "./+page.svelte";

function session(over: Partial<SessionInfo>): SessionInfo {
  return {
    id: "s1",
    cli: "claude",
    cwd: "C:\\Users\\me\\Project\\uninote",
    mode: "headless",
    title: "Fix the login bug",
    prompt: "fix the login bug",
    status: "done",
    pid: null,
    cliSessionId: "cli-1",
    startedAt: Date.now() - 60_000,
    endedAt: Date.now(),
    exitCode: 0,
    lastEvent: null,
    waiting: null,
    source: "manual",
    taskId: null,
    permissionMode: "ask",
    updatedAt: Date.now(),
    ...over,
  };
}

function serve(s: SessionInfo) {
  return phoneServer({
    [`GET /api/sessions/${s.id}`]: () => [200, { session: s, events: [], tail: s.mode === "interactive" ? "claude> ready" : "" }],
    [`POST /api/sessions/${s.id}/input`]: () => [200, { session: s }],
    [`POST /api/sessions/${s.id}/resume`]: () => [200, { session: { ...s, status: "running" } }],
  });
}

beforeEach(() => {
  phone.sessions = [];
  setUrl("/m/session?id=s1");
});

test("a finished headless session takes a follow-up from the phone", async () => {
  const fetchMock = serve(session({}));
  const user = userEvent.setup();
  render(Session);
  const box = await screen.findByLabelText("Message for Claude Code");
  expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
  await user.type(box, "now add a test");
  await user.click(screen.getByRole("button", { name: "Send" }));
  expect(sent(fetchMock, "POST /api/sessions/s1/input")).toEqual([{ text: "now add a test" }]);
  expect(box).toHaveValue("");
});

test("a follow-up is refused when the CLI gave no session id, with the reason", async () => {
  serve(session({ cliSessionId: null, cli: "codex" }));
  render(Session);
  expect(await screen.findByLabelText("Message for Codex CLI")).toBeDisabled();
  expect(screen.getByText(/did not report a session id/)).toBeInTheDocument();
});

test("a live terminal gets typed text and keys, and can be stopped", async () => {
  const s = session({ mode: "interactive", status: "running", endedAt: null });
  const fetchMock = serve(s);
  const user = userEvent.setup();
  render(Session);
  await user.click(await screen.findByRole("button", { name: "Press Escape" }));
  await user.type(screen.getByLabelText("Type into the terminal"), "yes{Enter}");
  expect(sent(fetchMock, "POST /api/sessions/s1/input")).toEqual([{ key: "esc" }, { text: "yes" }]);
  expect(screen.getByText("claude> ready")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Stop" }));
  expect(screen.getByRole("heading", { name: "Stop this session?" })).toBeInTheDocument();
});

test("the terminal keys are one row of caps, Tab included, each named for screen readers", async () => {
  const s = session({ mode: "interactive", status: "running", endedAt: null });
  const fetchMock = serve(s);
  const user = userEvent.setup();
  render(Session);
  const keys = within(await screen.findByRole("group", { name: "Terminal keys" })).getAllByRole("button");
  expect(keys.map((k) => k.getAttribute("aria-label"))).toEqual(["Press Escape", "Press Tab", "Arrow up", "Arrow down", "Press Enter", "Press Ctrl+C to interrupt"]);
  await user.click(screen.getByRole("button", { name: "Press Tab" }));
  await user.click(screen.getByRole("button", { name: "Press Enter" }));
  expect(sent(fetchMock, "POST /api/sessions/s1/input")).toEqual([{ key: "tab" }, { key: "enter" }]);
  // Stop and Send are icons alone, so their names are also their tooltips.
  for (const name of ["Stop", "Send"]) expect(screen.getByRole("button", { name })).toHaveAttribute("title", name);
});

test("a TUI's input box is drawn as a frame with its title, not as rows of box characters", async () => {
  const s = session({ mode: "interactive", status: "running", endedAt: null });
  const tail = "● Tests pass.\n╭─── Claude Code ───╮\n│ > add a test      │\n╰───────────────────╯";
  phoneServer({ [`GET /api/sessions/${s.id}`]: () => [200, { session: s, events: [], tail }] });
  render(Session);
  const pane = await screen.findByRole("region", { name: "Terminal screen" });
  expect(within(pane).getByText("Claude Code")).toHaveClass("m-box-title");
  expect(within(pane).getByText("> add a test")).toBeInTheDocument();
  expect(pane.textContent).not.toMatch(/[╭╮╰╯│─]/);
});

test("a scrolled-back terminal stays put and offers a jump to the new output", async () => {
  const s = session({ mode: "interactive", status: "running", endedAt: null });
  let tail = "first screen";
  phoneServer({ [`GET /api/sessions/${s.id}`]: () => [200, { session: s, events: [], tail }] });
  const user = userEvent.setup();
  render(Session);
  const pane = await screen.findByRole("region", { name: "Terminal screen" });
  Object.defineProperty(pane, "scrollHeight", { configurable: true, value: 1000 });
  Object.defineProperty(pane, "clientHeight", { configurable: true, value: 300 });
  pane.scrollTop = 100;
  await fireEvent.scroll(pane);
  expect(screen.getByRole("button", { name: "Jump to the latest output" })).toBeInTheDocument();

  tail = "second screen";
  receive({ type: "session", session: s });
  const jump = await screen.findByRole("button", { name: "New output below. Jump to it" }, { timeout: 2000 });
  expect(pane.scrollTop).toBe(100);
  await user.click(jump);
  expect(screen.queryByRole("button", { name: /Jump to/ })).not.toBeInTheDocument();
  expect(pane.scrollTop).toBe(1000);
});

test("Details shows what the desktop's side panel does: process, files changed and the Board card", async () => {
  const s = session({ status: "running", endedAt: null, pid: 4242, source: "board", taskId: "t1" });
  phoneServer({
    [`GET /api/sessions/${s.id}`]: () => [
      200,
      {
        session: s,
        events: [{ id: 1, sessionId: "s1", at: Date.now(), event: { kind: "tool_call", tool: "Bash", summary: "npm test" } }],
        tail: "",
        files: [["C:\\Users\\me\\Project\\uninote\\src\\login.ts", 2]],
        task: { id: "t1", title: "Fix the login bug", notes: "", project: "", cli: "claude", column: "progress", position: 1, sessionId: "s1", createdAt: 1, updatedAt: 1 },
        usage: { cpuPercent: 12.5, memoryBytes: 314572800, children: 3 },
      },
    ],
  });
  const user = userEvent.setup();
  render(Session);
  // A headless session opens on its steps.
  expect(await screen.findByText("• Bash npm test")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Activity" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryByRole("button", { name: "Terminal" })).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Details" }));
  const details = screen.getByRole("region", { name: "Details" });
  expect(within(details).getByText("4242")).toBeInTheDocument();
  expect(within(details).getByText("12.5%")).toBeInTheDocument();
  expect(within(details).getByText("300 MB")).toBeInTheDocument();
  expect(within(details).getByText("login.ts")).toBeInTheDocument();
  expect(within(details).getByText("2×")).toBeInTheDocument();
  expect(within(details).getByRole("link", { name: "Fix the login bug" })).toHaveAttribute("href", "/m/board?col=progress");
});

test("a session that cannot be loaded says why and offers Try again", async () => {
  phoneServer({ "GET /api/sessions/s1": () => [500, { error: "Database is locked." }] });
  render(Session);
  expect(await screen.findByRole("alert")).toHaveTextContent("The session could not be loaded: Database is locked.");
  expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
});

test("a closed terminal offers Resume instead of a text box", async () => {
  const fetchMock = serve(session({ mode: "interactive", status: "stopped" }));
  const user = userEvent.setup();
  render(Session);
  await user.click(await screen.findByRole("button", { name: "Resume terminal" }));
  expect(sent(fetchMock, "POST /api/sessions/s1/resume")).toHaveLength(1);
  expect(screen.queryByLabelText("Type into the terminal")).not.toBeInTheDocument();
});
