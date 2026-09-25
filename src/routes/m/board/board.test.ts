import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { SessionInfo, Task } from "$lib/api";
import { phone } from "$lib/phone.svelte";
import { setUrl } from "../../../test/app-state.svelte";
import { phoneServer, sent } from "../../../test/phone-server";
import Board from "./+page.svelte";

function task(over: Partial<Task>): Task {
  return {
    id: "t1",
    title: "Write the API docs",
    notes: "",
    project: "C:\\Users\\me\\Project\\uninote",
    cli: "claude",
    column: "todo",
    position: 1,
    sessionId: null,
    createdAt: 1,
    updatedAt: 1,
    ...over,
  };
}

beforeEach(() => {
  phone.sessions = [];
  vi.mocked(goto).mockClear();
  setUrl("/m/board");
});

test("one column at a time, with counts, and the choice kept in the address", async () => {
  phoneServer({
    "GET /api/tasks": () => [200, { tasks: [task({}), task({ id: "t2", title: "Ship the release", column: "done" })] }],
  });
  const u = userEvent.setup();
  render(Board);
  expect(await screen.findByText("Write the API docs")).toBeInTheDocument();
  expect(screen.queryByText("Ship the release")).not.toBeInTheDocument();
  await u.click(screen.getByRole("button", { name: "Done 1" }));
  expect(screen.getByText("Ship the release")).toBeInTheDocument();
  expect(goto).toHaveBeenCalledWith("/m/board?col=done", { replaceState: true, keepFocus: true, noScroll: true });
  await u.click(screen.getByRole("button", { name: "Pending 0" }));
  expect(screen.getByText(/Nothing pending/)).toBeInTheDocument();
});

test("a card opens a sheet to run it or move it", async () => {
  const fetchMock = phoneServer({
    "GET /api/tasks": () => [200, { tasks: [task({ notes: "Every endpoint, with examples" })] }],
    "POST /api/tasks/t1/move": () => [200, { task: task({ column: "pending" }) }],
  });
  const u = userEvent.setup();
  render(Board);
  await u.click(await screen.findByRole("button", { name: /Write the API docs/ }));
  const sheet = screen.getByRole("dialog");
  expect(within(sheet).getByText("Every endpoint, with examples")).toBeInTheDocument();
  expect(within(sheet).getByRole("link", { name: "Run in uninote" })).toHaveAttribute("href", "/m/new?task=t1");
  await u.click(within(sheet).getByRole("button", { name: "Pending" }));
  expect(sent(fetchMock, "POST /api/tasks/t1/move")).toEqual([{ column: "pending" }]);
});

test("a card whose session waits can be approved from the Board", async () => {
  const waiting: Partial<SessionInfo> = {
    id: "s1",
    status: "waiting",
    waiting: { reason: "permission", tool: "Bash", detail: "npm install", requestId: "r1", canAnswer: true, method: "stdio", since: Date.now() },
  };
  phone.sessions = [{ ...(waiting as SessionInfo), cli: "claude", cwd: "C:\\x", mode: "headless", title: "Docs" }];
  const fetchMock = phoneServer({
    "GET /api/tasks": () => [200, { tasks: [task({ column: "progress", sessionId: "s1" })] }],
    "POST /api/sessions/s1/answer": () => [200, { session: {} }],
  });
  const u = userEvent.setup();
  render(Board);
  await u.click(await screen.findByRole("button", { name: /In progress/ }));
  await u.click(screen.getByRole("button", { name: /Write the API docs/ }));
  const sheet = screen.getByRole("dialog");
  expect(within(sheet).getByText("Claude Code wants to run a command")).toBeInTheDocument();
  expect(within(sheet).queryByRole("link", { name: /Run/ })).not.toBeInTheDocument();
  await u.click(within(sheet).getByRole("button", { name: "Approve" }));
  expect(sent(fetchMock, "POST /api/sessions/s1/answer")).toEqual([{ allow: true }]);
});
