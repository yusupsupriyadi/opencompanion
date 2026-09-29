import { listen } from "@tauri-apps/api/event";
import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import type { ShellInfo, TerminalInfo } from "$lib/api";
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
  expect(screen.queryByRole("button", { name: "PowerShell" })).not.toBeInTheDocument();
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
  expect(document.querySelector(".panes")).toHaveClass("stacked");
  expect(border).toHaveAttribute("aria-orientation", "horizontal");
  border.focus();
  await user.keyboard("{ArrowDown}");
  expect(border).toHaveAttribute("aria-valuenow", "15");
  await user.click(stack);
  expect(stack).toHaveAttribute("aria-pressed", "false");
  expect(document.querySelector(".panes")).not.toHaveClass("stacked");
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
  expect(screen.queryByRole("button", { name: "PowerShell" })).not.toBeInTheDocument();
});
