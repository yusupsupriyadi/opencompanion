import { listen } from "@tauri-apps/api/event";
import { fireEvent, render, screen, within } from "@testing-library/svelte";
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
    get_session: () => ({ session: s, events: [], output: "" }),
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
  const calls = backend({ get_session: () => ({ session: s, events: [], output: "" }) });
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
  finish({ session: s, events: [stored] });
  const out = await screen.findByText(/Edit src\/app\.ts/);
  expect(out.closest("pre")).toHaveTextContent(/Read README\.md[\s\S]*Edit src\/app\.ts/);
  // The same event again, as a late listener would deliver it, is not shown twice.
  handlers["session-event"]({ payload: early });
  await Promise.resolve();
  expect(screen.getAllByText(/Edit src\/app\.ts/)).toHaveLength(1);
});

test("a terminal session has no header buttons: its shell starts the CLI again, and a closed one opens in place", async () => {
  const s = session({ mode: "interactive", status: "idle" });
  app.sessions = [s];
  const calls = backend({
    get_session: () => ({ session: s, events: [], output: "" }),
    session_output: () => ({ data: "", seq: 0 }),
    resume_session: () => ({ ...s, status: "running" }),
  });
  const { unmount } = render(SessionPage);

  expect(await screen.findByRole("heading", { level: 1, name: "Add a dark mode toggle" })).toBeInTheDocument();
  for (const name of ["Stop", "Mark done", "Resume", "Open terminal"]) {
    expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
  }
  expect(screen.queryByRole("navigation", { name: "Breadcrumb" })).not.toBeInTheDocument();
  expect(document.querySelector("#session-term .term-note")).toHaveTextContent("When Claude Code exits, type claude to start it again, or exit to close the terminal.");
  unmount();

  app.sessions = [{ ...s, status: "done", endedAt: Date.now() }];
  render(SessionPage);
  expect(await screen.findByText(/This terminal is closed/)).toBeInTheDocument();
  await fireEvent.click(screen.getByRole("button", { name: "Open terminal" }));
  expect(calls.calls("resume_session")).toEqual([{ id: "s1", cols: 120, rows: 32 }]);
});

test("a running headless session is stopped from its output bar, after a confirmation", async () => {
  const s = session({ status: "running" });
  app.sessions = [s];
  const calls = backend({ get_session: () => ({ session: s, events: [], output: "" }), stop_session: () => undefined });
  render(SessionPage);

  const bar = (await screen.findByPlaceholderText(/Claude Code/)).closest("form")!;
  await fireEvent.click(within(bar).getByRole("button", { name: "Stop" }));
  const dialog = await screen.findByRole("dialog");
  await fireEvent.click(within(dialog).getByRole("button", { name: "Stop session" }));
  expect(calls.calls("stop_session")).toEqual([{ id: "s1" }]);
});

test("Details says what the header used to: CLI, mode, permissions, folder and start", async () => {
  const s = session({ status: "done", endedAt: Date.now(), permissionMode: "bypass", source: "chat" });
  app.sessions = [s];
  backend({ get_session: () => ({ session: s, events: [], output: "" }) });
  render(SessionPage);

  await fireEvent.click(await screen.findByRole("tab", { name: "Details" }));
  const about = screen.getByRole("heading", { name: "Session" }).closest(".side-group")!;
  expect(about).toHaveTextContent("Modeheadless");
  expect(about).toHaveTextContent("PermissionsBypass");
  expect(about).toHaveTextContent("Started fromChat");
  expect(within(about as HTMLElement).getByText(/uninote/)).toHaveClass("mono");
});
