import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { SessionInfo } from "$lib/api";
import { phone } from "$lib/phone.svelte";
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

test("a closed terminal offers Resume instead of a text box", async () => {
  const fetchMock = serve(session({ mode: "interactive", status: "stopped" }));
  const user = userEvent.setup();
  render(Session);
  await user.click(await screen.findByRole("button", { name: "Resume terminal" }));
  expect(sent(fetchMock, "POST /api/sessions/s1/resume")).toHaveLength(1);
  expect(screen.queryByLabelText("Type into the terminal")).not.toBeInTheDocument();
});
