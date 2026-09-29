import { listen } from "@tauri-apps/api/event";
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import type { ShellInfo, TerminalInfo } from "$lib/api";
import ContextMenu from "$lib/ContextMenu.svelte";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend as mockBackend, session } from "../../test/fixtures";
import SessionPage from "./+page.svelte";

const SHELLS: ShellInfo[] = [
  { id: "pwsh", label: "PowerShell", path: String.raw`C:\Program Files\PowerShell\7\pwsh.exe` },
  { id: "cmd", label: "Command Prompt", path: String.raw`C:\Windows\System32\cmd.exe` },
];

function term(over: Partial<TerminalInfo> = {}): TerminalInfo {
  return {
    id: "t1",
    sessionId: "s1",
    cwd: String.raw`C:\Users\me\Project\uninote`,
    shell: "pwsh",
    shellLabel: "PowerShell",
    pid: 7,
    running: true,
    exitCode: null,
    startedAt: 1000,
    ...over,
  };
}

/** A finished headless session; every mounted terminal reads its screen so far, empty here. */
function backend(handlers: Parameters<typeof mockBackend>[0]) {
  const s = session({ status: "done", endedAt: Date.now() });
  app.sessions = [s];
  return mockBackend({
    get_session: () => ({ session: s, events: [] }),
    terminal_output: () => ({ data: "", seq: 0 }),
    ...handlers,
  });
}

beforeEach(() => {
  vi.mocked(listen).mockImplementation(async () => () => undefined);
  app.clis = CLIS;
  app.sessions = [];
  app.now = Date.now();
  setUrl("/session?id=s1");
});

test("New terminal opens the chosen shell in the session's folder and shows its tab", async () => {
  const user = userEvent.setup();
  const calls = backend({
    terminal_list: () => [],
    terminal_shells: () => SHELLS,
    terminal_open: (a) => term({ id: "new", shell: String(a?.shell), shellLabel: "Command Prompt" }),
  });
  render(SessionPage);

  const own = await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  expect(own).toHaveAttribute("aria-current", "true");
  await user.click(screen.getByRole("button", { name: "New terminal" }));
  const dialog = await screen.findByRole("dialog", { name: "New terminal" });
  expect(within(dialog).getByText(String.raw`C:\Users\me\Project\uninote`)).toBeInTheDocument();
  const shell = await within(dialog).findByRole("combobox", { name: "Shell" });
  expect(shell).toHaveValue("pwsh");
  await user.selectOptions(shell, "cmd");
  expect(within(dialog).getByText(String.raw`C:\Windows\System32\cmd.exe`)).toBeInTheDocument();
  await user.click(within(dialog).getByRole("button", { name: "Open in uninote" }));

  expect(calls.calls("terminal_open")[0]).toMatchObject({ sessionId: "s1", shell: "cmd" });
  const tab = await screen.findByRole("button", { name: "Command Prompt" });
  expect(tab).toHaveAttribute("aria-current", "true");
  expect(own).not.toHaveAttribute("aria-current");
  expect(document.getElementById("session-term")).not.toBeVisible();
  expect(document.getElementById("shell-panel-new")).toBeVisible();
  // The next terminal starts in the default shell again, not in the one picked last.
  await user.click(screen.getByRole("button", { name: "New terminal" }));
  expect(await screen.findByRole("combobox", { name: "Shell" })).toHaveValue("pwsh");
});

test("shell tabs sit beside the session's own tab, and shells of one kind are numbered", async () => {
  const user = userEvent.setup();
  const calls = backend({ terminal_list: () => [term({ id: "a" }), term({ id: "b" }), term({ id: "c", shell: "cmd", shellLabel: "Command Prompt" })] });
  render(SessionPage);

  const first = await screen.findByRole("button", { name: "PowerShell" });
  const second = screen.getByRole("button", { name: "PowerShell 2" });
  expect(screen.getByRole("button", { name: "Command Prompt" })).toBeInTheDocument();
  expect(calls.calls("terminal_list")[0]).toEqual({ sessionId: "s1" });
  expect(document.getElementById("session-term")).toBeVisible();
  expect(document.getElementById("shell-panel-a")).not.toBeVisible();

  await user.click(second);
  expect(second).toHaveAttribute("aria-current", "true");
  expect(document.getElementById("shell-panel-b")).toBeVisible();
  expect(document.getElementById("session-term")).not.toBeVisible();
  await user.click(first);
  expect(document.getElementById("shell-panel-a")).toBeVisible();
  expect(document.getElementById("shell-panel-b")).not.toBeVisible();
});

test("closing a shell tab ends it and shows the next one, and the last one gives way to the session", async () => {
  const user = userEvent.setup();
  const calls = backend({ terminal_list: () => [term({ id: "a" }), term({ id: "b", shell: "cmd", shellLabel: "Command Prompt" })], terminal_close: () => undefined });
  render(SessionPage);

  await user.click(await screen.findByRole("button", { name: "PowerShell" }));
  await user.click(screen.getByRole("button", { name: "Close terminal: PowerShell" }));
  expect(calls.calls("terminal_close")[0]).toEqual({ id: "a" });
  expect(screen.queryByRole("button", { name: "PowerShell" })).not.toBeInTheDocument();
  const next = screen.getByRole("button", { name: "Command Prompt" });
  expect(next).toHaveAttribute("aria-current", "true");
  expect(next).toHaveFocus();

  await user.click(screen.getByRole("button", { name: "Close terminal: Command Prompt" }));
  const own = screen.getByRole("button", { name: /^Claude Code\s*output$/ });
  expect(own).toHaveAttribute("aria-current", "true");
  expect(own).toHaveFocus();
  expect(document.getElementById("session-term")).toBeVisible();
});

test("a shell that exited says so and can be restarted", async () => {
  const user = userEvent.setup();
  const calls = backend({
    terminal_list: () => [term({ running: false, pid: null, exitCode: 1 })],
    terminal_restart: () => term({ startedAt: 2000 }),
  });
  render(SessionPage);

  await user.click(await screen.findByRole("button", { name: /^PowerShell\s*Exited$/ }));
  expect(screen.getByText("PowerShell exited with code 1.")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Restart" }));

  expect(calls.calls("terminal_restart")[0]).toEqual({ id: "t1" });
  expect(await screen.findByText(/pastes text, or an image/)).toBeInTheDocument();
  expect(screen.queryByText("PowerShell exited with code 1.")).not.toBeInTheDocument();
});

test("a shell that exits while the session is open shows as exited, and other sessions' shells stay out", async () => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  vi.mocked(listen).mockImplementation(async (name, handler) => {
    handlers[name] = handler as (e: { payload: unknown }) => void;
    return () => undefined;
  });
  backend({ terminal_list: () => [term()] });
  render(SessionPage);

  await screen.findByRole("button", { name: "PowerShell" });
  handlers["terminal-changed"]({ payload: term({ id: "other", sessionId: "s2", shell: "cmd", shellLabel: "Command Prompt" }) });
  handlers["terminal-changed"]({ payload: term({ running: false, pid: null, exitCode: 0 }) });
  expect(await screen.findByText("PowerShell exited with code 0.")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Command Prompt" })).not.toBeInTheDocument();
});

test("shells that fail to load say why and can be tried again", async () => {
  const user = userEvent.setup();
  let fail = true;
  backend({ terminal_list: () => (fail ? new Error("boom") : [term()]) });
  render(SessionPage);

  const alert = (await screen.findByText("This session's terminals could not be loaded: boom")).closest("p")!;
  fail = false;
  await user.click(within(alert).getByRole("button", { name: "Try again" }));
  expect(await screen.findByRole("button", { name: "PowerShell" })).toBeInTheDocument();
  expect(screen.queryByText(/could not be loaded/)).not.toBeInTheDocument();
});

test("the hint names the paste key of the OS the app runs on", async () => {
  const ua = vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15");
  backend({ terminal_list: () => [term()] });
  render(SessionPage);
  await screen.findByRole("button", { name: "PowerShell" });
  const note = document.querySelector("#shell-panel-t1 .term-note");
  expect(note).toHaveTextContent("Ctrl+Shift+V pastes text");
  expect(note?.textContent).not.toContain("{paste}");
  ua.mockRestore();
});

/** A finished headless session of its own: a session's split layout is kept for as long as the window is open. */
function splitBackend(sid: string, handlers: Parameters<typeof mockBackend>[0] = {}) {
  const s = session({ id: sid, status: "done", endedAt: Date.now() });
  app.sessions = [s];
  setUrl(`/session?id=${sid}`);
  let n = 0;
  return mockBackend({
    get_session: () => ({ session: s, events: [] }),
    terminal_output: () => ({ data: "", seq: 0 }),
    terminal_list: () => [],
    terminal_shells: () => SHELLS,
    terminal_open: () => term({ id: `${sid}-t${++n}`, sessionId: sid }),
    terminal_close: () => undefined,
    ...handlers,
  });
}

async function splitOnce(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Split terminal" }));
  const dialog = await screen.findByRole("dialog", { name: "Split terminal" });
  await within(dialog).findByRole("combobox", { name: "Shell" });
  await user.click(within(dialog).getByRole("button", { name: "Open in uninote" }));
}

test("Split terminal opens a shell beside the session's own terminal, in the same tab", async () => {
  const user = userEvent.setup();
  const calls = splitBackend("sp-a");
  render(SessionPage);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  expect(screen.queryByRole("button", { name: "Stack terminals" })).not.toBeInTheDocument();

  await splitOnce(user);
  expect(calls.calls("terminal_open")[0]).toMatchObject({ sessionId: "sp-a", shell: "pwsh" });
  const tab = await screen.findByRole("button", { name: /^Claude Code\s*output\s*with 1 more terminal$/ });
  expect(tab).toHaveAttribute("aria-current", "true");
  expect(document.getElementById("shell-tab-sp-a-t1")).toBeNull();
  expect(document.getElementById("session-term")).toBeVisible();
  expect(document.getElementById("shell-panel-sp-a-t1")).toBeVisible();
  expect(screen.getByRole("button", { name: "Close terminal: PowerShell" }).closest(".pane-head")).not.toBeNull();

  expect(screen.getByRole("separator", { name: "Resize Claude Code and PowerShell" })).toBeInTheDocument();
  // One keyboard note under the split, for the terminal last focused: the new shell took focus when it opened.
  expect(document.querySelector("#shell-panel-sp-a-t1 .term-note")).toBeNull();
  expect(document.querySelector("#session-panes > .term-note")).toHaveTextContent("Ctrl+C copies selected text");
});

test("the border between split terminals moves with the arrow keys along it, and Stack terminals turns it", async () => {
  const user = userEvent.setup();
  splitBackend("sp-b");
  render(SessionPage);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);

  const border = await screen.findByRole("separator");
  expect(border).toHaveAttribute("aria-orientation", "vertical");
  expect(border).toHaveAttribute("aria-valuenow", "50");
  border.focus();
  await user.keyboard("{ArrowRight}");
  expect(border).toHaveAttribute("aria-valuenow", "55");
  await user.keyboard("{ArrowDown}");
  expect(border).toHaveAttribute("aria-valuenow", "55");
  await user.keyboard("{Home}");
  expect(border).toHaveAttribute("aria-valuenow", "10");

  const stack = screen.getByRole("button", { name: "Stack terminals" });
  expect(stack).toHaveAttribute("aria-pressed", "false");
  await user.click(stack);
  expect(stack).toHaveAttribute("aria-pressed", "true");
  const shell = document.getElementById("shell-panel-sp-b-t1")!;
  expect(shell.style.top).toBe("10%");
  expect(shell.style.left).toBe("0%");
  expect(border).toHaveAttribute("aria-orientation", "horizontal");
  border.focus();
  await user.keyboard("{ArrowDown}");
  expect(border).toHaveAttribute("aria-valuenow", "15");
  await user.click(stack);
  expect(stack).toHaveAttribute("aria-pressed", "false");
  expect(shell.style.left).toBe("15%");
  expect(shell.style.top).toBe("0%");
});

test("closing a split terminal from its header gives the tab back to the session's own terminal", async () => {
  const user = userEvent.setup();
  const calls = splitBackend("sp-c");
  render(SessionPage);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);

  await user.click(await screen.findByRole("button", { name: "Close terminal: PowerShell" }));
  expect(calls.calls("terminal_close")[0]).toEqual({ id: "sp-c-t1" });
  const own = screen.getByRole("button", { name: /^Claude Code\s*output$/ });
  expect(own).toHaveAttribute("aria-current", "true");
  expect(own).toHaveFocus();
  expect(screen.queryByRole("separator")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Stack terminals" })).not.toBeInTheDocument();
});

test("a shell's tab splits up to four terminals, each closed from its own header", async () => {
  const user = userEvent.setup();
  splitBackend("sp-d", { terminal_list: () => [term({ id: "sp-d-a", sessionId: "sp-d" })] });
  render(SessionPage);
  await user.click(await screen.findByRole("button", { name: "PowerShell" }));
  for (let i = 0; i < 3; i++) await splitOnce(user);

  const tab = await screen.findByRole("button", { name: /^PowerShell\s*with 3 more terminals$/ });
  expect(tab).toHaveAttribute("aria-current", "true");
  const closes = screen.getAllByRole("button", { name: /^Close terminal: / });
  expect(closes.map((b) => b.getAttribute("aria-label"))).toEqual(["Close terminal: PowerShell", "Close terminal: PowerShell 2", "Close terminal: PowerShell 3", "Close terminal: PowerShell 4"]);
  expect(closes.every((b) => b.closest(".pane-head"))).toBe(true);
  const split = screen.getByRole("button", { name: "Split terminal" });
  expect(split).toBeDisabled();
  expect(split).toHaveAttribute("title", "A tab holds up to 4 terminals");
});

test("a split is still there when the session is opened again", async () => {
  const user = userEvent.setup();
  let listed: TerminalInfo[] = [];
  splitBackend("sp-e", { terminal_list: () => listed });
  const first = render(SessionPage);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);
  await screen.findByRole("button", { name: /with 1 more terminal$/ });
  first.unmount();

  listed = [term({ id: "sp-e-t1", sessionId: "sp-e" })];
  render(SessionPage);
  expect(await screen.findByRole("button", { name: /^Claude Code\s*output\s*with 1 more terminal$/ })).toBeInTheDocument();
  expect(document.getElementById("shell-tab-sp-e-t1")).toBeNull();
});

const tabNames = () => [...document.querySelectorAll("#session-views .tab-pick b")].map((b) => b.textContent);
const paneOrder = () => [...document.querySelectorAll("#session-panes .pane:not([hidden])")].map((p) => p.id);

/** Right-clicks `target` and picks `item` from the menu that opens. */
async function fromMenu(user: ReturnType<typeof userEvent.setup>, target: HTMLElement, item: string) {
  await fireEvent.contextMenu(target);
  await user.click(screen.getByRole("menuitem", { name: item }));
}

test("a tab's right-click menu moves it along the tabs, or splits a lone terminal into another tab", async () => {
  const user = userEvent.setup();
  splitBackend("mv-a", {
    terminal_list: () => [term({ id: "mv-a-a", sessionId: "mv-a" }), term({ id: "mv-a-b", sessionId: "mv-a", shell: "cmd", shellLabel: "Command Prompt" })],
  });
  render(SessionPage);
  render(ContextMenu);
  const ps = await screen.findByRole("button", { name: "PowerShell" });

  // Shift+F10 opens it on the focused tab, and focus comes back to that tab after the move.
  ps.focus();
  await fireEvent.contextMenu(ps);
  expect(screen.getByRole("menu", { name: "PowerShell tab" })).toBeInTheDocument();
  const items = screen.getAllByRole("menuitem").map((m) => m.textContent?.trim());
  expect(items.slice(0, 13)).toEqual([
    "Close tab Ctrl+Shift+W",
    "Close other tabs",
    "Close tabs to the right",
    "Close all tabs",
    "Rename tab",
    "Pin tab",
    "Duplicate tab",
    "Clear terminal",
    "Restart terminal",
    "Move tab left",
    "Move tab right",
    "Split with Claude Code",
    "Split with Command Prompt",
  ]);
  // Then the session folder's own entries, as in the sidebar.
  expect(items.slice(13)).toHaveLength(2);
  expect(items[13]).toBe("Copy folder path");
  await user.click(screen.getByRole("menuitem", { name: "Move tab right" }));
  expect(tabNames()).toEqual(["Claude Code", "Command Prompt", "PowerShell"]);
  expect(ps).toHaveFocus();

  await fromMenu(user, ps, "Split with Claude Code");
  expect(tabNames()).toEqual(["Claude Code", "Command Prompt"]);
  expect(screen.getByRole("button", { name: /^Claude Code\s*output\s*with 1 more terminal$/ })).toHaveAttribute("aria-current", "true");
  expect(paneOrder()).toEqual(["session-term", "shell-panel-mv-a-a"]);
});

test("a split terminal swaps, docks along an edge and moves out to a tab of its own from its header", async () => {
  const user = userEvent.setup();
  splitBackend("mv-b");
  render(SessionPage);
  render(ContextMenu);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);

  const name = await screen.findByRole("button", { name: "PowerShell" });
  expect(name).toHaveAttribute("aria-haspopup", "menu");
  await user.click(name);
  expect(screen.getAllByRole("menuitem").map((m) => m.textContent?.trim())).toEqual([
    "Swap with Claude Code",
    "Move to the left edge",
    "Move to the right edge",
    "Move to the top edge",
    "Move to the bottom edge",
    "Move to a new tab",
  ]);
  await user.click(screen.getByRole("menuitem", { name: "Swap with Claude Code" }));
  expect(paneOrder()).toEqual(["shell-panel-mv-b-t1", "session-term"]);
  expect(screen.getByRole("button", { name: /^PowerShell\s*with 1 more terminal$/ })).toHaveAttribute("aria-current", "true");

  await user.click(screen.getByRole("button", { name: "PowerShell" }));
  await user.click(screen.getByRole("menuitem", { name: "Move to the bottom edge" }));
  expect(paneOrder()).toEqual(["session-term", "shell-panel-mv-b-t1"]);
  expect(screen.getByRole("separator")).toHaveAttribute("aria-orientation", "horizontal");

  await user.click(screen.getByRole("button", { name: "PowerShell" }));
  await user.click(screen.getByRole("menuitem", { name: "Move to a new tab" }));
  expect(tabNames()).toEqual(["Claude Code", "PowerShell"]);
  const own = screen.getByRole("button", { name: "PowerShell" });
  expect(own).toHaveAttribute("aria-current", "true");
  expect(own).toHaveFocus();
  expect(paneOrder()).toEqual(["shell-panel-mv-b-t1"]);
});

/** Drags from `from` to a point over `over`, which `elementFromPoint` reports there; jsdom has no layout. */
async function dragTo(from: HTMLElement, over: Element, x: number, y = 10) {
  document.elementFromPoint = () => over;
  await fireEvent.pointerDown(from, { button: 0, clientX: 0, clientY: 0 });
  await fireEvent.pointerMove(window, { clientX: x, clientY: y });
}

test("a tab dragged onto another tab joins it, and one dragged to a tab's edge moves between tabs", async () => {
  splitBackend("mv-c", {
    terminal_list: () => [term({ id: "mv-c-a", sessionId: "mv-c" }), term({ id: "mv-c-b", sessionId: "mv-c", shell: "cmd", shellLabel: "Command Prompt" })],
  });
  render(SessionPage);
  const ps = await screen.findByRole("button", { name: "PowerShell" });
  const own = document.querySelector<HTMLElement>('[data-tab="term"]')!;
  own.getBoundingClientRect = () => ({ left: 0, top: 0, width: 100, height: 38, right: 100, bottom: 38, x: 0, y: 0, toJSON: () => ({}) });

  // The left quarter of a tab lands before it.
  await dragTo(ps, own, 10);
  expect(document.querySelector(".drag-ghost")).toHaveTextContent("PowerShell");
  expect(own).toHaveClass("drop-before");
  await fireEvent.pointerUp(window, { clientX: 10, clientY: 10 });
  expect(tabNames()).toEqual(["PowerShell", "Claude Code", "Command Prompt"]);
  expect(document.querySelector(".drag-ghost")).toBeNull();

  // Its middle joins it.
  const cmd = screen.getByRole("button", { name: "Command Prompt" });
  await dragTo(cmd, own, 50);
  expect(own).toHaveClass("drop-join");
  await fireEvent.pointerUp(window, { clientX: 50, clientY: 10 });
  expect(tabNames()).toEqual(["PowerShell", "Claude Code"]);
  expect(screen.getByRole("button", { name: /^Claude Code\s*output\s*with 1 more terminal$/ })).toHaveAttribute("aria-current", "true");
});

test("a terminal dragged beside a shown one splits the tab that way, and Escape leaves everything as it was", async () => {
  splitBackend("mv-d", { terminal_list: () => [term({ id: "mv-d-a", sessionId: "mv-d" })] });
  render(SessionPage);
  const ps = await screen.findByRole("button", { name: "PowerShell" });
  const pane = document.getElementById("session-term")!;
  pane.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 400, right: 400, bottom: 400, x: 0, y: 0, toJSON: () => ({}) });

  await dragTo(ps, pane, 200, 380);
  expect(pane.querySelector(".drop-zone")).toHaveClass("bottom");
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(pane.querySelector(".drop-zone")).toBeNull();
  expect(tabNames()).toEqual(["Claude Code", "PowerShell"]);

  await dragTo(ps, pane, 20, 200);
  expect(pane.querySelector(".drop-zone")).toHaveClass("left");
  await fireEvent.pointerUp(window, { clientX: 20, clientY: 200 });
  expect(tabNames()).toEqual(["PowerShell"]);
  expect(paneOrder()).toEqual(["shell-panel-mv-d-a", "session-term"]);
  expect(screen.getByRole("separator")).toHaveAttribute("aria-orientation", "vertical");
});

test("the border between split terminals follows the pointer, and Escape puts it back", async () => {
  const user = userEvent.setup();
  splitBackend("mv-e");
  render(SessionPage);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);
  const box = document.querySelector<HTMLElement>(".panes")!;
  box.getBoundingClientRect = () => ({ left: 0, top: 0, width: 1000, height: 600, right: 1000, bottom: 600, x: 0, y: 0, toJSON: () => ({}) });
  const border = await screen.findByRole("separator");

  await fireEvent.pointerDown(border, { button: 0, clientX: 500, clientY: 300 });
  await fireEvent.pointerMove(window, { clientX: 600, clientY: 300 });
  expect(border).toHaveAttribute("aria-valuenow", "60");
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(border).toHaveAttribute("aria-valuenow", "50");

  await fireEvent.pointerDown(border, { button: 0, clientX: 500, clientY: 300 });
  await fireEvent.pointerMove(window, { clientX: 50, clientY: 300 });
  await fireEvent.pointerUp(window, { clientX: 50, clientY: 300 });
  // No terminal gets narrower than 120 px.
  expect(border).toHaveAttribute("aria-valuenow", "12");
  await fireEvent.pointerMove(window, { clientX: 900, clientY: 300 });
  expect(border).toHaveAttribute("aria-valuenow", "12");
});

test("terminals dropped on different sides nest splits: one beside, then one under it", async () => {
  splitBackend("mv-f", {
    terminal_list: () => [term({ id: "mv-f-a", sessionId: "mv-f" }), term({ id: "mv-f-b", sessionId: "mv-f", shell: "cmd", shellLabel: "Command Prompt" })],
  });
  render(SessionPage);
  const ps = await screen.findByRole("button", { name: "PowerShell" });
  const box = { left: 0, top: 0, width: 400, height: 400, right: 400, bottom: 400, x: 0, y: 0, toJSON: () => ({}) };
  const own = document.getElementById("session-term")!;
  own.getBoundingClientRect = () => box;
  await dragTo(ps, own, 380, 200);
  await fireEvent.pointerUp(window, { clientX: 380, clientY: 200 });

  const a = document.getElementById("shell-panel-mv-f-a")!;
  a.getBoundingClientRect = () => box;
  await dragTo(screen.getByRole("button", { name: "Command Prompt" }), a, 200, 390);
  expect(a.querySelector(".drop-zone")).toHaveClass("bottom");
  await fireEvent.pointerUp(window, { clientX: 200, clientY: 390 });

  expect(tabNames()).toEqual(["Claude Code"]);
  expect(paneOrder()).toEqual(["session-term", "shell-panel-mv-f-a", "shell-panel-mv-f-b"]);
  const b = document.getElementById("shell-panel-mv-f-b")!;
  expect([b.style.left, b.style.top, b.style.width, b.style.height]).toEqual(["50%", "50%", "50%", "50%"]);
  expect(screen.getAllByRole("separator").map((s) => s.getAttribute("aria-orientation"))).toEqual(["vertical", "horizontal"]);
  expect(screen.getByRole("separator", { name: "Resize PowerShell and Command Prompt" })).toBeInTheDocument();
});

test("right-click in a terminal offers copy, paste, a new tab and a split on three sides", async () => {
  const user = userEvent.setup();
  const calls = splitBackend("rc-a", { terminal_list: () => [term({ id: "rc-a-a", sessionId: "rc-a" })], terminal_write: () => undefined });
  render(SessionPage);
  render(ContextMenu);
  await user.click(await screen.findByRole("button", { name: "PowerShell" }));
  const host = () => document.querySelector<HTMLElement>("#shell-panel-rc-a-a .xterm-host")!;

  await fireEvent.contextMenu(host());
  expect(screen.getByRole("menu", { name: "PowerShell" })).toBeInTheDocument();
  expect(screen.getAllByRole("menuitem").map((m) => m.textContent?.trim())).toEqual([
    "Copy",
    "Paste",
    "Clear terminal",
    "New terminal tab",
    "Split right",
    "Split left",
    "Split down",
    "Restart terminal",
  ]);
  // Nothing is selected, so there is nothing to copy.
  expect(screen.getByRole("menuitem", { name: "Copy" })).toHaveAttribute("aria-disabled", "true");

  // Paste types the clipboard's text into the shell.
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { readText: async () => "npm test", writeText: async () => undefined } });
  await user.click(screen.getByRole("menuitem", { name: "Paste" }));
  await vi.waitFor(() => expect(calls.calls("terminal_write").map((c) => c.data).join("")).toContain("npm test"));

  await fireEvent.contextMenu(host());
  await user.click(screen.getByRole("menuitem", { name: "Split left" }));
  const dialog = await screen.findByRole("dialog", { name: "Split terminal" });
  await within(dialog).findByRole("combobox", { name: "Shell" });
  await user.click(within(dialog).getByRole("button", { name: "Open in uninote" }));

  expect(await screen.findByRole("button", { name: /^PowerShell 2\s*with 1 more terminal$/ })).toHaveAttribute("aria-current", "true");
  expect(paneOrder()).toEqual(["shell-panel-rc-a-t1", "shell-panel-rc-a-a"]);
  expect(screen.getByRole("separator")).toHaveAttribute("aria-orientation", "vertical");

  await fireEvent.contextMenu(host());
  await user.click(screen.getByRole("menuitem", { name: "New terminal tab" }));
  expect(await screen.findByRole("dialog", { name: "New terminal" })).toBeInTheDocument();
});

test("a split down from the right-click menu opens under that terminal, in the tab it is in", async () => {
  const user = userEvent.setup();
  splitBackend("rc-b");
  render(SessionPage);
  render(ContextMenu);
  await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await splitOnce(user);
  // The session's own output has no xterm; its shell does.
  await fireEvent.contextMenu(document.querySelector<HTMLElement>("#shell-panel-rc-b-t1 .xterm-host")!);
  await user.click(screen.getByRole("menuitem", { name: "Split down" }));
  const dialog = await screen.findByRole("dialog", { name: "Split terminal" });
  await within(dialog).findByRole("combobox", { name: "Shell" });
  await user.click(within(dialog).getByRole("button", { name: "Open in uninote" }));

  await screen.findByRole("button", { name: /with 2 more terminals$/ });
  const under = document.getElementById("shell-panel-rc-b-t2")!;
  expect([under.style.left, under.style.top, under.style.width, under.style.height]).toEqual(["50%", "50%", "50%", "50%"]);
  expect(screen.getAllByRole("separator").map((s) => s.getAttribute("aria-orientation"))).toEqual(["vertical", "horizontal"]);
});

function threeShells(sid: string) {
  return splitBackend(sid, {
    terminal_list: () => [
      term({ id: `${sid}-a`, sessionId: sid }),
      term({ id: `${sid}-b`, sessionId: sid, shell: "cmd", shellLabel: "Command Prompt" }),
      term({ id: `${sid}-c`, sessionId: sid, shell: "bash", shellLabel: "Git Bash" }),
    ],
  });
}

test("Close other tabs asks first, then ends every shell outside the tab, and the session's own terminal stays", async () => {
  const user = userEvent.setup();
  const calls = threeShells("cl-a");
  render(SessionPage);
  render(ContextMenu);
  const cmd = await screen.findByRole("button", { name: "Command Prompt" });

  await fromMenu(user, cmd, "Close other tabs");
  const dialog = await screen.findByRole("dialog", { name: "Close 2 terminals?" });
  expect(within(dialog).getByText("PowerShell, Git Bash stop, with everything started in them.")).toBeInTheDocument();
  await user.click(within(dialog).getByRole("button", { name: "Keep them open" }));
  expect(calls.calls("terminal_close")).toEqual([]);

  await fromMenu(user, cmd, "Close other tabs");
  await user.click(await screen.findByRole("button", { name: "Close 2 terminals" }));
  expect(calls.calls("terminal_close")).toEqual([{ id: "cl-a-a" }, { id: "cl-a-c" }]);
  expect(tabNames()).toEqual(["Claude Code", "Command Prompt"]);
});

test("the session's own tab has nothing of its own to close, and Close all tabs ends every shell", async () => {
  const user = userEvent.setup();
  const calls = threeShells("cl-b");
  render(SessionPage);
  render(ContextMenu);
  const own = await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await screen.findByRole("button", { name: "Git Bash" });

  await fireEvent.contextMenu(own);
  const close = screen.getByRole("menuitem", { name: /^Close tab(\s|$)/ });
  expect(close).toHaveAttribute("aria-disabled", "true");
  expect(close).toHaveTextContent("The session's own terminal closes with exit");
  await user.click(screen.getByRole("menuitem", { name: "Close all tabs" }));
  await user.click(await screen.findByRole("button", { name: "Close 3 terminals" }));
  expect(calls.calls("terminal_close").map((c) => c.id)).toEqual(["cl-b-a", "cl-b-b", "cl-b-c"]);
  expect(tabNames()).toEqual(["Claude Code"]);
  expect(screen.getByRole("button", { name: /^Claude Code\s*output$/ })).toHaveFocus();
});

test("a pinned tab moves first, shows a pin for its close, and the bulk closes leave it open", async () => {
  const user = userEvent.setup();
  const calls = threeShells("cl-c");
  render(SessionPage);
  render(ContextMenu);
  const bash = await screen.findByRole("button", { name: "Git Bash" });

  await fromMenu(user, bash, "Pin tab");
  expect(tabNames()).toEqual(["Git Bash", "Claude Code", "PowerShell", "Command Prompt"]);
  const pinned = screen.getByRole("button", { name: /^Git Bash\s*, pinned$/ });
  expect(screen.queryByRole("button", { name: "Close terminal: Git Bash" })).not.toBeInTheDocument();
  expect(pinned.closest(".tab")?.querySelector(".tab-pin")).not.toBeNull();

  // It moves only among the pinned tabs.
  await fireEvent.contextMenu(pinned);
  expect(screen.getByRole("menuitem", { name: "Move tab right" })).toHaveAttribute("aria-disabled", "true");
  await user.click(screen.getByRole("menuitem", { name: "Close all tabs" }));
  await user.click(await screen.findByRole("button", { name: "Close 2 terminals" }));
  expect(calls.calls("terminal_close").map((c) => c.id)).toEqual(["cl-c-a", "cl-c-b"]);
  expect(tabNames()).toEqual(["Git Bash", "Claude Code"]);

  await fromMenu(user, pinned, "Unpin tab");
  expect(tabNames()).toEqual(["Git Bash", "Claude Code"]);
  expect(screen.getByRole("button", { name: "Close terminal: Git Bash" })).toBeInTheDocument();
  // Its own menu still closes it, pinned or not.
  await fromMenu(user, screen.getByRole("button", { name: "Git Bash" }), "Close tab Ctrl+Shift+W");
  expect(calls.calls("terminal_close").map((c) => c.id)).toEqual(["cl-c-a", "cl-c-b", "cl-c-c"]);
});

test("a tab is renamed in place: Enter keeps the name, Escape drops it, and an empty one gives the shell's back", async () => {
  const user = userEvent.setup();
  threeShells("tb-a");
  render(SessionPage);
  render(ContextMenu);
  const ps = await screen.findByRole("button", { name: "PowerShell" });

  await fromMenu(user, ps, "Rename tab");
  const field = screen.getByRole("textbox", { name: "Tab name" });
  expect(field).toHaveFocus();
  await user.type(field, "dev server{Enter}");
  const renamed = screen.getByRole("button", { name: "dev server" });
  expect(renamed).toHaveFocus();
  expect(tabNames()).toEqual(["Claude Code", "dev server", "Command Prompt", "Git Bash"]);

  // Double-click also renames; Escape leaves it as it was.
  await user.dblClick(renamed);
  await user.type(screen.getByRole("textbox", { name: "Tab name" }), " 2{Escape}");
  expect(screen.getByRole("button", { name: "dev server" })).toBeInTheDocument();

  await user.dblClick(screen.getByRole("button", { name: "dev server" }));
  await user.clear(screen.getByRole("textbox", { name: "Tab name" }));
  await user.keyboard("{Enter}");
  expect(screen.getByRole("button", { name: "PowerShell" })).toBeInTheDocument();
});

test("Close tabs to the right ends the shells after the tab, and Duplicate opens the same shell beside it", async () => {
  const user = userEvent.setup();
  const calls = threeShells("tb-b");
  render(SessionPage);
  render(ContextMenu);
  const cmd = await screen.findByRole("button", { name: "Command Prompt" });

  await fromMenu(user, cmd, "Duplicate tab");
  expect(calls.calls("terminal_open")[0]).toMatchObject({ sessionId: "tb-b", shell: "cmd" });
  await vi.waitFor(() => expect(tabNames()).toEqual(["Claude Code", "PowerShell", "Command Prompt", "PowerShell 2", "Git Bash"]));

  await fromMenu(user, screen.getByRole("button", { name: "Command Prompt" }), "Close tabs to the right");
  await user.click(await screen.findByRole("button", { name: "Close 2 terminals" }));
  expect(calls.calls("terminal_close").map((c) => c.id)).toEqual(["tb-b-t1", "tb-b-c"]);
  expect(tabNames()).toEqual(["Claude Code", "PowerShell", "Command Prompt"]);
});

test("Clear forgets a shell's output and Restart starts it again, from the tab or the terminal", async () => {
  const user = userEvent.setup();
  const calls = splitBackend("tb-c", {
    terminal_list: () => [term({ id: "tb-c-a", sessionId: "tb-c" })],
    terminal_clear: () => undefined,
    terminal_restart: () => term({ id: "tb-c-a", sessionId: "tb-c", startedAt: 2000 }),
  });
  render(SessionPage);
  render(ContextMenu);
  const ps = await screen.findByRole("button", { name: "PowerShell" });

  await fromMenu(user, ps, "Clear terminal");
  expect(calls.calls("terminal_clear")).toEqual([{ id: "tb-c-a" }]);
  await user.click(ps);
  await fireEvent.contextMenu(document.querySelector<HTMLElement>("#shell-panel-tb-c-a .xterm-host")!);
  await user.click(screen.getByRole("menuitem", { name: "Restart terminal" }));
  expect(calls.calls("terminal_restart")).toEqual([{ id: "tb-c-a" }]);
});

test("Ctrl+PageDown and Ctrl+PageUp switch tabs from anywhere, and Ctrl+Shift+W closes the shown one", async () => {
  const user = userEvent.setup();
  const calls = threeShells("tb-d");
  render(SessionPage);
  const own = await screen.findByRole("button", { name: /^Claude Code\s*output$/ });
  await screen.findByRole("button", { name: "Git Bash" });

  await user.keyboard("{Control>}{PageDown}{/Control}");
  expect(screen.getByRole("button", { name: "PowerShell" })).toHaveAttribute("aria-current", "true");
  expect(document.activeElement?.closest("#shell-panel-tb-d-a")).not.toBeNull();
  await user.keyboard("{Control>}{PageUp}{PageUp}{/Control}");
  expect(screen.getByRole("button", { name: "Git Bash" })).toHaveAttribute("aria-current", "true");

  await user.keyboard("{Control>}{Shift>}W{/Shift}{/Control}");
  expect(calls.calls("terminal_close")).toEqual([{ id: "tb-d-c" }]);
  expect(tabNames()).toEqual(["Claude Code", "PowerShell", "Command Prompt"]);
  // The session's own tab has nothing to close.
  own.click();
  await user.keyboard("{Control>}{Shift>}W{/Shift}{/Control}");
  expect(calls.calls("terminal_close")).toHaveLength(1);
});
