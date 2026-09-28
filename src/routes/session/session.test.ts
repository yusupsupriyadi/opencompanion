import { listen } from "@tauri-apps/api/event";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";
import type { EventRow } from "$lib/api";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend as mockBackend, session } from "../../test/fixtures";
import SessionPage from "./+page.svelte";

/** These sessions have no shell open beside them. */
function backend(handlers: Parameters<typeof mockBackend>[0]) {
  return mockBackend({ terminal_list: () => [], ...handlers });
}

beforeEach(() => {
  app.clis = CLIS;
  app.sessions = [];
  app.now = Date.now();
  setUrl("/session?id=s1");
});

test("a running session shows the CPU and memory of its CLI and what it started", async () => {
  const s = session({ status: "running" });
  app.sessions = [s];
  const calls = backend({
    get_session: () => ({ session: s, events: [], output: "", task: null }),
    session_usage: () => ({ cpuPercent: 12.5, memoryBytes: 1.5 * 1073741824, children: 3 }),
  });
  render(SessionPage);

  expect(await screen.findByText("12.5%")).toBeInTheDocument();
  expect(screen.getByText("1.5 GB")).toBeInTheDocument();
  expect(screen.getByText("Child processes").nextElementSibling).toHaveTextContent("3");
  expect(calls.calls("session_usage")[0]).toEqual({ id: "s1" });
});

test("a finished session is not measured", async () => {
  const s = session({ status: "done", endedAt: Date.now() });
  app.sessions = [s];
  const calls = backend({ get_session: () => ({ session: s, events: [], output: "", task: null }) });
  render(SessionPage);

  expect(await screen.findByRole("heading", { name: "Add a dark mode toggle" })).toBeInTheDocument();
  expect(screen.queryByText("CPU")).not.toBeInTheDocument();
  expect(calls.calls("session_usage")).toHaveLength(0);
});

test("events that arrive while the page loads are kept, once each", async () => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  vi.mocked(listen).mockImplementation(async (name, handler) => {
    handlers[name] = handler as (e: { payload: unknown }) => void;
    return () => undefined;
  });
  const s = session({ status: "running" });
  app.sessions = [s];
  const stored: EventRow = { id: 1, sessionId: "s1", at: Date.now() - 1000, event: { kind: "tool_call", tool: "Read", summary: "README.md" } };
  const early: EventRow = { id: 2, sessionId: "s1", at: Date.now(), event: { kind: "tool_call", tool: "Edit", summary: "src/app.ts" } };
  let finish: (d: unknown) => void = () => undefined;
  backend({ get_session: () => new Promise((resolve) => (finish = resolve)) });
  render(SessionPage);

  await vi.waitFor(() => expect(handlers["session-event"]).toBeTypeOf("function"));
  handlers["session-event"]({ payload: early });
  finish({ session: s, events: [stored], task: null });
  const out = await screen.findByText(/Edit src\/app\.ts/);
  expect(out.closest("pre")).toHaveTextContent(/Read README\.md[\s\S]*Edit src\/app\.ts/);
  // The same event again, as a late listener would deliver it, is not shown twice.
  handlers["session-event"]({ payload: early });
  await Promise.resolve();
  expect(screen.getAllByText(/Edit src\/app\.ts/)).toHaveLength(1);
});

test("an idle terminal can be marked done, and a working one only stopped", async () => {
  const s = session({ mode: "interactive", status: "idle" });
  app.sessions = [s];
  const calls = backend({
    get_session: () => ({ session: s, events: [], output: "", task: null }),
    session_output: () => ({ data: "", seq: 0 }),
  });
  const { unmount } = render(SessionPage);

  const done = await screen.findByRole("button", { name: "Mark done" });
  expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
  await fireEvent.click(done);
  expect(calls.calls("mark_session_done")).toEqual([{ id: "s1" }]);
  unmount();

  app.sessions = [{ ...s, status: "running" }];
  render(SessionPage);
  expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Mark done" })).not.toBeInTheDocument();
});
