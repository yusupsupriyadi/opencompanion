import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { Task } from "$lib/api";
import { i18n } from "$lib/i18n.svelte";
import { app, toast } from "$lib/store.svelte";
import { CLIS, backend, session } from "../../test/fixtures";
import Board from "./+page.svelte";

const task = (over: Partial<Task>): Task => ({
  id: "t1",
  title: "Add week view",
  notes: "",
  project: "C:\\p\\calendar-project",
  cli: null,
  column: "pending",
  position: 1,
  sessionId: null,
  createdAt: 1,
  updatedAt: 1,
  ...over,
});

let tasks: Task[] = [];

beforeEach(() => {
  app.clis = CLIS;
  app.sessions = [];
  tasks = [
    task({ id: "t1", title: "Decide on the export format", column: "pending", notes: "Open question" }),
    task({ id: "t2", title: "Fix the failing tests", column: "todo", cli: "codex", notes: "Run npm test until green", project: "C:\\p\\uninote" }),
    task({ id: "t3", title: "Write API docs", column: "progress", cli: "claude", sessionId: "s1" }),
  ];
});

function api() {
  return backend({
    list_tasks: () => tasks,
    recent_projects: () => [],
    save_task: (a) => {
      const t = a?.task as Task;
      return { ...task({}), ...t, id: t.id ?? "new" };
    },
    delete_task: () => undefined,
  });
}

test("columns show their cards, counts and empty hints", async () => {
  api();
  render(Board);
  const todo = await screen.findByRole("region", { name: /Todo/ });
  expect(within(todo).getByRole("button", { name: "Fix the failing tests" })).toBeInTheDocument();
  expect(screen.getByText("Finished cards land here.")).toBeInTheDocument();
});

test("a running session shows on its card, a waiting one gets the accent", async () => {
  api();
  app.sessions = [session({ id: "s1", status: "waiting", waiting: { reason: "permission", tool: "Bash", detail: "x", requestId: null, canAnswer: true, method: "hook", since: Date.now() } })];
  render(Board);
  const card = (await screen.findByRole("button", { name: "Write API docs" })).closest(".tcard");
  expect(card).toHaveClass("needs");
  expect(within(card as HTMLElement).getByText("Waiting for you")).toBeInTheDocument();
  expect(within(card as HTMLElement).getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/session?id=s1");
});

test("a new card needs a title, then lands in Pending", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(Board);
  await user.click(await screen.findByRole("button", { name: "New card" }));
  await user.click(screen.getByRole("button", { name: "Save card" }));
  expect(screen.getByText("Give the card a title.")).toBeInTheDocument();
  expect(calls.calls("save_task")).toHaveLength(0);
  await user.type(screen.getByLabelText("Title"), "Add CSV export");
  await user.click(screen.getByRole("button", { name: "Save card" }));
  expect(calls.calls("save_task")[0]).toMatchObject({ task: { title: "Add CSV export", column: "pending" } });
  expect(toast.text).toBe('Added "Add CSV export" to Pending.');
});

test("the Column field is the keyboard way to move a card", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(Board);
  await user.click(await screen.findByRole("button", { name: "Decide on the export format" }));
  await user.selectOptions(screen.getByLabelText("Column"), "todo");
  await user.click(screen.getByRole("button", { name: "Save card" }));
  expect(calls.calls("save_task")[0]).toMatchObject({ task: { id: "t1", column: "todo" } });
  expect(toast.text).toBe('Moved "Decide on the export format" to Todo.');
});

test("delete asks twice", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(Board);
  await user.click(await screen.findByRole("button", { name: "Decide on the export format" }));
  await user.click(screen.getByRole("button", { name: "Delete" }));
  expect(calls.calls("delete_task")).toHaveLength(0);
  await user.click(screen.getByRole("button", { name: "Press again to delete" }));
  expect(calls.calls("delete_task")).toEqual([{ id: "t1" }]);
});

test("in Indonesian the columns, buttons and toasts use the Indonesian words", async () => {
  api();
  const user = userEvent.setup();
  i18n.lang = "id";
  try {
    render(Board);
    expect(await screen.findByRole("region", { name: /Tertunda/ })).toBeInTheDocument();
    expect(screen.getByText("Kartu yang selesai masuk ke sini.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Kartu baru" }));
    await user.type(screen.getByLabelText("Judul"), "Tambah ekspor CSV");
    await user.click(screen.getByRole("button", { name: "Simpan kartu" }));
    expect(toast.text).toBe('"Tambah ekspor CSV" ditambahkan ke Tertunda.');
  } finally {
    i18n.lang = "en";
  }
});

test("Run on a Todo card opens the run dialog filled from the card", async () => {
  api();
  const user = userEvent.setup();
  render(Board);
  await user.click(await screen.findByRole("button", { name: "Run in uninote" }));
  expect(screen.getByRole("heading", { name: "Run this card" })).toBeInTheDocument();
  expect(screen.getByLabelText(/Codex CLI/)).toBeChecked();
  expect(screen.getByLabelText("First message (optional)")).toHaveValue("Run npm test until green");
  expect(screen.getByRole("button", { name: "Start Codex CLI in uninote" })).toBeInTheDocument();
});
