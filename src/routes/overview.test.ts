import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import NewSessionDialog from "$lib/NewSessionDialog.svelte";
import { app } from "$lib/store.svelte";
import { CLIS, backend, session } from "../test/fixtures";
import Overview from "./+page.svelte";

beforeEach(() => {
  app.clis = CLIS;
  app.sessionsState = "ready";
  app.outsideState = "ready";
  app.outside = [];
  backend({ recent_projects: () => [] });
});

test("first run explains what to do and offers both ways in", async () => {
  app.sessions = [];
  const user = userEvent.setup();
  render(Overview);
  render(NewSessionDialog);
  expect(screen.getByRole("heading", { name: "Nothing has run in OpenCompanion yet" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Describe a task in Chat" })).toHaveAttribute("href", "/chat");
  await user.click(screen.getAllByRole("button", { name: "New session" })[0]);
  expect(screen.getByRole("heading", { name: "New session" })).toBeInTheDocument();
});

test("a waiting session is the focal point, running and finished ones follow", () => {
  app.sessions = [
    session({ id: "w", status: "waiting", waiting: { reason: "permission", tool: "Write", detail: "a.txt", requestId: "r", canAnswer: true, method: "stdio", since: Date.now() } }),
    session({ id: "r", title: "Fix tests", status: "running" }),
    session({ id: "d", title: "Docs", status: "done", endedAt: Date.now() }),
  ];
  render(Overview);
  expect(screen.getByRole("heading", { name: "Claude Code wants to change a file" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /Fix tests/ })).toHaveAttribute("href", "/session?id=r");
  expect(screen.getByRole("link", { name: /Docs/ })).toHaveAttribute("href", "/session?id=d");
  expect(screen.getByText("1 running, 1 finished today")).toBeInTheDocument();
});

test("with nothing from today, the hint links to all sessions inside its sentence", () => {
  const old = Date.now() - 3 * 86_400_000;
  app.sessions = [session({ id: "old", status: "done", startedAt: old, endedAt: old })];
  render(Overview);
  const hint = screen.getByText(/Nothing is running right now/);
  expect(hint).toHaveTextContent("Nothing is running right now. Earlier sessions are under All sessions.");
  expect(within(hint).getByRole("link", { name: "All sessions" })).toHaveAttribute("href", "/history");
});

test("outside sessions are read-only rows with real process data", () => {
  app.sessions = [session({ id: "r", status: "running" })];
  app.outside = [{ pid: 37100, kind: "claude", mode: "interactive", cwd: "C:\\Users\\me\\Project\\ai-remote", startedAt: Math.floor(Date.now() / 1000) - 600, cpuPercent: 1, memoryBytes: 500 * 1048576 }];
  render(Overview);
  expect(screen.getByText("PID 37100 · 500 MB")).toBeInTheDocument();
  expect(screen.getByText("Read-only")).toBeInTheDocument();
});
