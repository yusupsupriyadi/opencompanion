import { fireEvent, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { setUrl } from "../test/app-state.svelte";
import { session } from "../test/fixtures";
import ContextMenu from "./ContextMenu.svelte";
import { closeMenu } from "./context-menu.svelte";
import SessionRow from "./SessionRow.svelte";
import Sidebar from "./Sidebar.svelte";
import { app, pendingDelete, pendingNew, toast } from "./store.svelte";

const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
const itemNames = (menu: HTMLElement) => within(menu).getAllByRole("menuitem").map((b) => b.textContent?.trim());

beforeEach(() => {
  closeMenu(false);
  app.sessions = [];
  pendingDelete.session = null;
  pendingNew.open = false;
  for (const key of ["air-collapsed-folders", "air-pinned-folders", "air-pinned-sessions"]) localStorage.removeItem(key);
  setUrl("/");
  vi.mocked(goto).mockClear();
  vi.mocked(revealItemInDir).mockClear();
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(WINDOWS_UA);
});

afterEach(() => {
  vi.restoreAllMocks();
});

function sidebar(...sessions: ReturnType<typeof session>[]) {
  app.sessions = sessions;
  render(Sidebar);
  render(ContextMenu);
}

test("right-clicking a session in the sidebar opens its menu instead of the webview's", async () => {
  sidebar(session({ id: "a", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }));
  const link = screen.getByRole("link", { name: /Write API docs/ });

  // fireEvent returns false when the default (the webview's own menu) was prevented.
  expect(await fireEvent.contextMenu(link)).toBe(false);
  const menu = screen.getByRole("menu", { name: "Session: Write API docs" });
  expect(itemNames(menu)).toEqual(["Open session", "New session in uninote", "Pin session", "Copy folder path", "Show in File Explorer", "Delete session"]);
  expect(within(menu).getByRole("menuitem", { name: "Open session" })).toHaveFocus();
  expect(link.closest(".mini-item")).toHaveClass("menu-open");
});

test("Delete opens the confirm dialog; a live session's Delete says to stop it first and does nothing", async () => {
  const user = userEvent.setup();
  sidebar(session({ id: "a", title: "Write API docs", status: "done" }), session({ id: "b", title: "Fix the login bug", status: "running" }));

  await fireEvent.contextMenu(screen.getByRole("link", { name: /Fix the login bug/ }));
  const refused = screen.getByRole("menuitem", { name: /Delete session/ });
  expect(refused).toHaveAttribute("aria-disabled", "true");
  expect(refused).toHaveTextContent("Stop it first");
  await user.click(refused);
  expect(pendingDelete.session).toBeNull();
  expect(screen.getByRole("menu")).toBeInTheDocument();

  await fireEvent.contextMenu(screen.getByRole("link", { name: /Write API docs/ }));
  await user.click(screen.getByRole("menuitem", { name: "Delete session" }));
  expect(pendingDelete.session?.id).toBe("a");
  expect(screen.queryByRole("menu")).toBeNull();
});

test("the arrow keys, Home and End move through the menu; Escape closes it and returns to the row", async () => {
  const user = userEvent.setup();
  sidebar(session({ id: "a", title: "Write API docs", status: "done" }));
  const link = screen.getByRole("link", { name: /Write API docs/ });
  link.focus();
  // What Shift+F10 or the Menu key fires on the focused row.
  await fireEvent.contextMenu(link);
  const items = screen.getAllByRole("menuitem");

  expect(items[0]).toHaveFocus();
  await user.keyboard("{ArrowUp}");
  expect(items.at(-1)).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(items[0]).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(items[1]).toHaveFocus();
  await user.keyboard("{End}");
  expect(items.at(-1)).toHaveFocus();
  await user.keyboard("{Home}");
  expect(items[0]).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("menu")).toBeNull();
  expect(link).toHaveFocus();
});

test("Pin in the menu pins the session, and the next menu offers Unpin", async () => {
  const user = userEvent.setup();
  sidebar(session({ id: "a", title: "Write API docs", status: "done" }));
  const link = screen.getByRole("link", { name: /Write API docs/ });

  await fireEvent.contextMenu(link);
  await user.click(screen.getByRole("menuitem", { name: "Pin session" }));
  expect(link).toHaveAccessibleName(/, Pinned$/);
  expect(JSON.parse(localStorage.getItem("air-pinned-sessions") ?? "[]")).toEqual(["a"]);

  await fireEvent.contextMenu(link);
  expect(screen.getByRole("menuitem", { name: "Unpin session" })).toBeInTheDocument();
});

test("a folder's menu collapses it, copies its path and shows it in File Explorer", async () => {
  const user = userEvent.setup();
  sidebar(session({ id: "a", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }));
  const head = screen.getByRole("button", { name: "uninote" });

  await fireEvent.contextMenu(head);
  const menu = screen.getByRole("menu", { name: "Folder: uninote" });
  expect(itemNames(menu)).toEqual(["New session in uninote", "Pin folder", "Collapse folder", "Copy folder path", "Show in File Explorer"]);
  await user.click(within(menu).getByRole("menuitem", { name: "Collapse folder" }));
  expect(head).toHaveAttribute("aria-expanded", "false");

  await fireEvent.contextMenu(head);
  await user.click(screen.getByRole("menuitem", { name: "Copy folder path" }));
  await vi.waitFor(() => expect(toast.text).toBe("Copied C:\\p\\uninote"));
  expect(await navigator.clipboard.readText()).toBe("C:\\p\\uninote");

  await fireEvent.contextMenu(head);
  await user.click(screen.getByRole("menuitem", { name: "Show in File Explorer" }));
  expect(revealItemInDir).toHaveBeenCalledWith("C:\\p\\uninote");
});

test("a folder the file manager cannot open says so", async () => {
  const user = userEvent.setup();
  vi.mocked(revealItemInDir).mockRejectedValueOnce("The system cannot find the path specified.");
  sidebar(session({ id: "a", title: "Write API docs", cwd: "C:\\p\\gone", status: "done" }));

  await fireEvent.contextMenu(screen.getByRole("button", { name: "gone" }));
  await user.click(screen.getByRole("menuitem", { name: "Show in File Explorer" }));
  await vi.waitFor(() => expect(toast.text).toBe("Could not open gone: The system cannot find the path specified."));
});

test("a session row in Overview or History has the same menu without pins", async () => {
  const user = userEvent.setup();
  render(SessionRow, { s: session({ id: "a", title: "Write API docs", cwd: "C:\\p\\uninote", status: "done" }) });
  render(ContextMenu);
  const link = screen.getByRole("link", { name: /Write API docs/ });

  await fireEvent.contextMenu(link);
  expect(itemNames(screen.getByRole("menu", { name: "Session: Write API docs" }))).toEqual([
    "Open session",
    "New session in uninote",
    "Copy folder path",
    "Show in File Explorer",
    "Delete session",
  ]);
  expect(link.closest(".srow-item")).toHaveClass("menu-open");
  await user.click(screen.getByRole("menuitem", { name: "Open session" }));
  expect(goto).toHaveBeenCalledWith("/session?id=a");

  await fireEvent.contextMenu(link);
  await user.click(screen.getByRole("menuitem", { name: "New session in uninote" }));
  expect(pendingNew).toMatchObject({ open: true, cwd: "C:\\p\\uninote" });
});

test("a click elsewhere, a scroll or leaving the window closes the menu", async () => {
  sidebar(session({ id: "a", title: "Write API docs", status: "done" }));
  const link = screen.getByRole("link", { name: /Write API docs/ });

  await fireEvent.contextMenu(link);
  await fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("menu")).toBeNull();

  await fireEvent.contextMenu(link);
  await fireEvent.scroll(window);
  expect(screen.queryByRole("menu")).toBeNull();

  await fireEvent.contextMenu(link);
  await fireEvent.blur(window);
  expect(screen.queryByRole("menu")).toBeNull();
  expect(link.closest(".mini-item")).not.toHaveClass("menu-open");
});
