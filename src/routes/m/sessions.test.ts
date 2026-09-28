import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { SessionInfo } from "$lib/api";
import { phone } from "$lib/phone.svelte";
import { phoneServer, sent } from "../../test/phone-server";
import Sessions from "./+page.svelte";

function session(over: Partial<SessionInfo>): SessionInfo {
  return {
    id: "s1",
    cli: "claude",
    cwd: "C:\\Users\\me\\Project\\uninote",
    mode: "headless",
    title: "Fix the login bug",
    prompt: "fix the login bug",
    status: "running",
    pid: 10,
    cliSessionId: "cli-1",
    startedAt: Date.now() - 60_000,
    endedAt: null,
    exitCode: null,
    lastEvent: null,
    waiting: null,
    source: "manual",
    permissionMode: "ask",
    updatedAt: Date.now(),
    ...over,
  };
}

beforeEach(() => {
  phone.loaded = true;
  phone.connection = "online";
  phone.sessions = [];
});

test("New session is a plus icon that still says what it does", () => {
  phone.sessions = [session({})];
  render(Sessions);
  const add = screen.getByRole("link", { name: "New session" });
  expect(add).toHaveAttribute("href", "/m/new");
  expect(add).toHaveAttribute("title", "New session");
  expect(screen.getByRole("status")).toHaveTextContent("Connected");
});

test("an empty list gives the one action that fills it", () => {
  render(Sessions);
  expect(screen.getByText(/Nothing has run in OpenCompanion yet/)).toBeInTheDocument();
  expect(screen.getAllByRole("link", { name: "New session" })).toHaveLength(2);
});

test("a waiting session is answered in place, and opens from an icon", async () => {
  phone.sessions = [
    session({
      id: "w1",
      status: "waiting",
      waiting: { reason: "permission", tool: "Bash", detail: "npm install", requestId: "r1", canAnswer: true, method: "stdio", since: Date.now() },
    }),
    session({ id: "r1", title: "Write the API docs" }),
  ];
  const fetchMock = phoneServer({ "POST /api/sessions/w1/answer": () => [200, { session: {} }] });
  const user = userEvent.setup();
  render(Sessions);
  const card = screen.getByRole("region", { name: "Claude Code wants to run a command" });
  expect(within(card).getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/m/session?id=w1");
  await user.click(within(card).getByRole("button", { name: "Approve" }));
  expect(sent(fetchMock, "POST /api/sessions/w1/answer")).toEqual([{ allow: true }]);

  const running = screen.getByRole("region", { name: "Running · 1" });
  expect(within(running).getByRole("link", { name: /Write the API docs/ })).toHaveAttribute("href", "/m/session?id=r1");
  expect(within(running).getByText("Running")).toHaveClass("m-status");
});
