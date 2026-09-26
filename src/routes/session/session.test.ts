import { render, screen } from "@testing-library/svelte";
import { beforeEach, expect, test } from "vitest";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend, session } from "../../test/fixtures";
import SessionPage from "./+page.svelte";

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
