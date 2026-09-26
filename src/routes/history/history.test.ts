import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import type { SessionQuery } from "$lib/api";
import { app } from "$lib/store.svelte";
import { CLIS, backend, session } from "../../test/fixtures";
import History from "./+page.svelte";

const older = [
  session({ id: "a", title: "Fix the login bug", status: "done", startedAt: Date.now() - 3 * 86_400_000, endedAt: Date.now() - 3 * 86_400_000 }),
  session({ id: "b", cli: "codex", title: "Write the API docs", status: "error", startedAt: Date.now() - 5 * 86_400_000, endedAt: Date.now() - 5 * 86_400_000 }),
];

beforeEach(() => {
  app.clis = CLIS;
  app.sessions = [];
  app.now = Date.now();
});

test("every session is listed, and the search and filters go to the backend", async () => {
  const asked: SessionQuery[] = [];
  const calls = backend({
    search_sessions: (a) => {
      const q = a?.query as SessionQuery;
      asked.push(q);
      return older.filter((s) => (!q.text || s.title.toLowerCase().includes(q.text.toLowerCase())) && (!q.cli || s.cli === q.cli));
    },
    session_folders: () => [String.raw`C:\Users\me\Project\uninote`],
  });
  const user = userEvent.setup();
  render(History);

  const found = await screen.findByRole("region", { name: "Sessions found" });
  expect(within(found).getByRole("link", { name: /Fix the login bug/ })).toHaveAttribute("href", "/session?id=a");
  expect(within(found).getByRole("link", { name: /Write the API docs/ })).toBeInTheDocument();
  expect(asked[0]).toEqual({ text: "", cli: null, folder: null, status: null, limit: 50, offset: 0 });

  await user.type(screen.getByRole("searchbox", { name: "Search sessions" }), "login");
  await screen.findByText("1 session found.");
  expect(screen.queryByRole("link", { name: /Write the API docs/ })).not.toBeInTheDocument();

  await user.clear(screen.getByRole("searchbox", { name: "Search sessions" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "CLI" }), "codex");
  await user.selectOptions(screen.getByRole("combobox", { name: "Status" }), "error");
  await vi.waitFor(() => expect(asked.at(-1)).toMatchObject({ text: "", cli: "codex", status: "error" }));
  expect(await screen.findByRole("link", { name: /Write the API docs/ })).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: /Fix the login bug/ })).not.toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Folder" })).toHaveTextContent("uninote");
  expect(calls.calls("session_folders")).toHaveLength(1);
});

test("a search with no match says what to do", async () => {
  backend({ search_sessions: () => [], session_folders: () => [] });
  const user = userEvent.setup();
  render(History);
  expect(await screen.findByText(/Nothing has run in OpenCompanion yet/)).toBeInTheDocument();
  await user.type(screen.getByRole("searchbox", { name: "Search sessions" }), "zzz");
  expect(await screen.findByText(/No session matches this search/)).toBeInTheDocument();
});
