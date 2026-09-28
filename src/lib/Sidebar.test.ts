import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { tick } from "svelte";
import { beforeEach, expect, test } from "vitest";
import { i18n } from "$lib/i18n.svelte";
import { setUrl } from "../test/app-state.svelte";
import { session } from "../test/fixtures";
import Sidebar from "./Sidebar.svelte";
import { app, pendingNew } from "./store.svelte";

beforeEach(() => {
  app.sessions = [];
  app.companion = null;
  pendingNew.open = false;
  localStorage.removeItem("air-collapsed-folders");
  localStorage.removeItem("air-pinned-folders");
  localStorage.removeItem("air-pinned-sessions");
  setUrl("/");
});

test("every nav item points at a screen that exists", () => {
  render(Sidebar);
  const nav = within(screen.getByRole("navigation", { name: "Main" }));
  const hrefs = nav.getAllByRole("link").map((a) => a.getAttribute("href"));
  expect(hrefs).toEqual(["/", "/board", "/chat", "/settings"]);
  expect(screen.getByRole("link", { name: "Overview" })).toHaveAttribute("aria-current", "page");
});

test("CLIs, Skills, Phone access and the theme are under Settings, not in the sidebar", () => {
  setUrl("/settings/skills");
  app.companion = { running: true, address: "192.168.1.5", port: 8765, error: null };
  render(Sidebar);
  expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("aria-current", "page");
  for (const name of [/^CLIs$/, /^Skills$/, /Phone access/]) expect(screen.queryByRole("link", { name })).toBeNull();
  expect(screen.queryByRole("button", { name: "Dusk" })).toBeNull();
});

test("waiting sessions come first in the session list, named by their task", () => {
  app.sessions = [
    session({ id: "a", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }),
    session({ id: "b", title: "Fix the login bug", cwd: "C:\\p\\uninote", status: "running" }),
    session({ id: "c", title: "Add week view", cwd: "C:\\p\\calendar", status: "waiting" }),
  ];
  render(Sidebar);
  const names = [...document.querySelectorAll(".mini b")].map((b) => b.textContent);
  expect(names).toEqual(["Add week view", "Fix the login bug", "Write API docs"]);
  expect(screen.getByRole("link", { name: /Add week view/ })).toHaveAttribute("href", "/session?id=c");
});

test("a session row shows only its provider mark and title; the running one shimmers", () => {
  app.sessions = [
    session({ id: "a", title: "Write API docs", status: "done" }),
    session({ id: "b", title: "Fix the login bug", status: "running", cli: "codex" }),
    session({ id: "c", title: "Add week view", status: "waiting" }),
  ];
  render(Sidebar);
  const row = screen.getByRole("link", { name: /Fix the login bug/ });
  expect(row.querySelector(".climark")).not.toBeNull();
  expect(row.querySelector(".dot")).toBeNull();
  // CLI and status are no longer printed, but screen readers and the tooltip still get them.
  expect(row).toHaveAccessibleName("Fix the login bug Codex CLI, Running");
  expect(row).toHaveAttribute("title", "Fix the login bug · Codex CLI · Running");
  const shimmering = [...document.querySelectorAll(".mini b.shimmer")].map((b) => b.textContent);
  expect(shimmering).toEqual(["Fix the login bug"]);
});

test("sessions are grouped under their folder, the folder with the most urgent session first", () => {
  app.sessions = [
    session({ id: "a", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }),
    session({ id: "b", title: "Add week view", cwd: "C:\\p\\calendar", status: "waiting" }),
    session({ id: "c", title: "Fix the login bug", cwd: "C:\\P\\Uninote\\", status: "running", cli: "codex" }),
    session({ id: "d", title: "Old calendar run", cwd: "C:\\q\\calendar", status: "stopped" }),
  ];
  render(Sidebar);
  // Same folder despite case and a trailing slash; same name in another path stays its own group.
  const groups = [...document.querySelectorAll<HTMLElement>(".folder-group")];
  expect(groups.map((g) => g.querySelector(".folder-head .mono")?.textContent)).toEqual(["calendar", "Uninote", "calendar"]);
  expect(screen.getAllByRole("group", { name: "calendar" })).toHaveLength(2);
  const titles = (g: HTMLElement) => [...g.querySelectorAll(".mini b")].map((b) => b.textContent);
  expect(titles(groups[0])).toEqual(["Add week view"]);
  expect(titles(groups[1])).toEqual(["Fix the login bug", "Write API docs"]);
  expect(titles(groups[2])).toEqual(["Old calendar run"]);
  expect(groups[2].querySelector(".folder-head")).toHaveAttribute("title", "C:\\q\\calendar");
  expect(screen.getByRole("link", { name: /Fix the login bug/ })).toHaveAccessibleName("Fix the login bug Codex CLI, Running");
});

test("a folder collapses to one line that still says how many sessions it holds and who is waiting", async () => {
  const user = userEvent.setup();
  app.sessions = [
    session({ id: "a", title: "Add week view", cwd: "C:\\p\\calendar", status: "waiting" }),
    session({ id: "b", title: "Fix the login bug", cwd: "C:\\p\\calendar", status: "running" }),
    session({ id: "c", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }),
  ];
  const { unmount } = render(Sidebar);
  const head = screen.getByRole("button", { name: /^calendar/ });
  expect(head).toHaveAttribute("aria-expanded", "true");

  await user.click(head);
  expect(head).toHaveAttribute("aria-expanded", "false");
  expect(screen.queryByRole("link", { name: /Add week view/ })).toBeNull();
  expect(head).toHaveAccessibleName("calendar Waiting for you 2 sessions");
  expect(screen.getByRole("link", { name: /Write API docs/ })).toBeVisible();

  // Stays collapsed the next time the sidebar mounts.
  unmount();
  render(Sidebar);
  const again = screen.getByRole("button", { name: /^calendar/ });
  expect(again).toHaveAttribute("aria-expanded", "false");
  await user.click(again);
  expect(screen.getByRole("link", { name: /Add week view/ })).toBeVisible();
  expect(again).toHaveAccessibleName("calendar");
});

test("New session opens from a folder with that folder, or from Sessions with none", async () => {
  const user = userEvent.setup();
  app.sessions = [session({ id: "a", title: "Add week view", cwd: "C:\\p\\calendar", status: "done" })];
  render(Sidebar);

  await user.click(screen.getByRole("button", { name: "New session in calendar" }));
  expect(pendingNew).toEqual({ open: true, cwd: "C:\\p\\calendar" });

  pendingNew.open = false;
  await user.click(screen.getByRole("button", { name: "New session" }));
  expect(pendingNew).toEqual({ open: true, cwd: "" });
});

test("with no sessions yet, Sessions still offers New session", () => {
  render(Sidebar);
  expect(screen.getByRole("button", { name: "New session" })).toBeInTheDocument();
  expect(document.querySelector(".folder-group")).toBeNull();
});

const groupNames = () => [...document.querySelectorAll(".folder-head .mono")].map((e) => e.textContent);

test("every session stays listed, however many there are", () => {
  app.sessions = Array.from({ length: 30 }, (_, n) =>
    session({ id: `s${n}`, title: `Task ${n}`, cwd: n % 2 ? "C:\\p\\uninote" : "C:\\p\\calendar", status: n === 29 ? "waiting" : "done" }),
  );
  render(Sidebar);
  const titles = [...document.querySelectorAll(".mini b")].map((b) => b.textContent);
  expect(titles).toHaveLength(30);
  expect(groupNames()).toEqual(["uninote", "calendar"]);
  expect(titles[0]).toBe("Task 29");
});

test("a pinned session comes first in its folder, after a restart", async () => {
  const user = userEvent.setup();
  const older = Array.from({ length: 6 }, (_, n) => session({ id: `o${n}`, title: `Older ${n}`, cwd: "C:\\p\\uninote", status: "done" }));
  app.sessions = older;
  const { unmount } = render(Sidebar);
  const pin = screen.getByRole("button", { name: "Pin session: Older 5" });
  expect(pin).toHaveAttribute("aria-pressed", "false");
  await user.click(pin);
  expect(pin).toHaveAttribute("aria-pressed", "true");

  unmount();
  const newer = Array.from({ length: 6 }, (_, n) => session({ id: `n${n}`, title: `Newer ${n}`, cwd: "C:\\p\\uninote", status: "done" }));
  app.sessions = [...newer, ...older];
  render(Sidebar);
  const titles = [...document.querySelectorAll(".mini b")].map((b) => b.textContent);
  expect(titles).toEqual(["Older 5", "Newer 0", "Newer 1", "Newer 2", "Newer 3", "Newer 4", "Newer 5", "Older 0", "Older 1", "Older 2", "Older 3", "Older 4"]);
  expect(screen.getByRole("button", { name: "Pin session: Older 5" })).toHaveAttribute("aria-pressed", "true");
});

test("a pinned folder stays on top, even with no recent session, ready for a new one", async () => {
  const user = userEvent.setup();
  const calendar = session({ id: "a", title: "Add week view", cwd: "C:\\p\\calendar", status: "waiting" });
  app.sessions = [calendar, session({ id: "b", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" })];
  const { unmount } = render(Sidebar);
  expect(groupNames()).toEqual(["calendar", "uninote"]);
  await user.click(screen.getByRole("button", { name: "Pin folder: uninote" }));
  expect(groupNames()).toEqual(["uninote", "calendar"]);

  // Its only session was deleted; the folder stays with an empty state and New session.
  unmount();
  app.sessions = [calendar];
  render(Sidebar);
  expect(groupNames()).toEqual(["uninote", "calendar"]);
  expect(screen.getByText("No recent sessions")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "New session in uninote" }));
  expect(pendingNew).toEqual({ open: true, cwd: "C:\\p\\uninote" });

  const unpin = screen.getByRole("button", { name: "Pin folder: uninote" });
  expect(unpin).toHaveAttribute("aria-pressed", "true");
  await user.click(unpin);
  expect(groupNames()).toEqual(["calendar"]);
});

test("switching the UI language to Indonesian relabels the open sidebar", async () => {
  app.sessions = [session({ id: "b", title: "Fix the login bug", status: "running", cli: "codex" })];
  render(Sidebar);
  expect(screen.getByRole("link", { name: "Overview" })).toBeInTheDocument();
  try {
    i18n.lang = "id";
    await tick();
    expect(screen.getByRole("link", { name: "Ringkasan" })).toHaveAttribute("href", "/");
    expect(screen.getByRole("button", { name: "Sesi baru" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Fix the login bug/ })).toHaveAccessibleName("Fix the login bug Codex CLI, Berjalan");
  } finally {
    i18n.lang = "en";
  }
});
