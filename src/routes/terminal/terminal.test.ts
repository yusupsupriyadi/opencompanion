import { listen } from "@tauri-apps/api/event";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import type { ShellInfo, TerminalInfo } from "$lib/api";
import { backend as mockBackend } from "../../test/fixtures";
import TerminalPage from "./+page.svelte";

const SHELLS: ShellInfo[] = [
  { id: "pwsh", label: "PowerShell", path: String.raw`C:\Program Files\PowerShell\7\pwsh.exe` },
  { id: "cmd", label: "Command Prompt", path: String.raw`C:\Windows\System32\cmd.exe` },
];

function term(over: Partial<TerminalInfo> = {}): TerminalInfo {
  return {
    id: "t1",
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

/** Every mounted terminal reads its screen so far; these tests start from an empty one. */
function backend(handlers: Parameters<typeof mockBackend>[0]) {
  return mockBackend({ terminal_output: () => ({ data: "", seq: 0 }), ...handlers });
}

beforeEach(() => {
  vi.mocked(listen).mockImplementation(async () => () => undefined);
});

test("with no terminal open the screen says what a terminal is for and opens one in the chosen folder and shell", async () => {
  const user = userEvent.setup();
  const calls = backend({
    terminal_list: () => [],
    terminal_shells: () => SHELLS,
    folder_exists: () => true,
    recent_projects: () => [],
    project_folders: () => [],
    terminal_open: (a) => term({ id: "new", cwd: String(a?.cwd), shell: String(a?.shell), shellLabel: "Command Prompt" }),
  });
  render(TerminalPage);

  expect(await screen.findByRole("heading", { name: "No terminal open" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "New terminal" }));
  const shell = await screen.findByRole("combobox", { name: "Shell" });
  expect(shell).toHaveValue("pwsh");
  await user.type(screen.getByRole("textbox", { name: "Project folder" }), String.raw`C:\Users\me\Project\api`);
  await user.selectOptions(shell, "cmd");
  expect(screen.getByText(String.raw`C:\Windows\System32\cmd.exe`)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Open in api" }));

  expect(calls.calls("terminal_open")[0]).toMatchObject({ cwd: String.raw`C:\Users\me\Project\api`, shell: "cmd" });
  const tab = await screen.findByRole("button", { name: /api\s*Command Prompt/ });
  expect(tab).toHaveAttribute("aria-current", "true");
  // The next terminal starts in the default shell again, not in the one picked last.
  await user.click(screen.getByRole("button", { name: "New terminal" }));
  expect(await screen.findByRole("combobox", { name: "Shell" })).toHaveValue("pwsh");
});

test("a missing folder opens nothing", async () => {
  const user = userEvent.setup();
  const calls = backend({ terminal_list: () => [], terminal_shells: () => SHELLS, folder_exists: () => false, recent_projects: () => [], project_folders: () => [] });
  render(TerminalPage);

  await user.click(await screen.findByRole("button", { name: "New terminal" }));
  await screen.findByRole("combobox", { name: "Shell" });
  await user.type(screen.getByRole("textbox", { name: "Project folder" }), String.raw`C:\nowhere`);
  await user.click(screen.getByRole("button", { name: "Open in nowhere" }));

  expect(await screen.findByText("This folder does not exist.")).toBeInTheDocument();
  expect(calls.calls("terminal_open")).toHaveLength(0);
});

test("tabs switch between terminals, and folders with the same name are numbered", async () => {
  const user = userEvent.setup();
  backend({
    terminal_list: () => [
      term({ id: "a" }),
      term({ id: "b", cwd: String.raw`D:\work\uninote`, shell: "cmd", shellLabel: "Command Prompt" }),
    ],
  });
  render(TerminalPage);

  const first = await screen.findByRole("button", { name: /^uninote\s*PowerShell/ });
  const second = screen.getByRole("button", { name: /^uninote 2\s*Command Prompt/ });
  expect(first).toHaveAttribute("aria-current", "true");
  expect(document.getElementById("term-panel-b")).not.toBeVisible();

  await user.click(second);
  expect(second).toHaveAttribute("aria-current", "true");
  expect(first).not.toHaveAttribute("aria-current");
  expect(document.getElementById("term-panel-b")).toBeVisible();
  expect(document.getElementById("term-panel-a")).not.toBeVisible();
});

test("closing a tab ends its terminal and shows the next one", async () => {
  const user = userEvent.setup();
  const calls = backend({ terminal_list: () => [term({ id: "a" }), term({ id: "b", cwd: String.raw`C:\p\api` })], terminal_close: () => undefined });
  render(TerminalPage);

  await user.click(await screen.findByRole("button", { name: "Close terminal: uninote" }));
  expect(calls.calls("terminal_close")[0]).toEqual({ id: "a" });
  expect(screen.queryByRole("button", { name: /^uninote/ })).not.toBeInTheDocument();
  const api = screen.getByRole("button", { name: /^api/ });
  expect(api).toHaveAttribute("aria-current", "true");
  expect(api).toHaveFocus();
});

test("a shell that exited says so and can be restarted", async () => {
  const user = userEvent.setup();
  const calls = backend({
    terminal_list: () => [term({ running: false, pid: null, exitCode: 1 })],
    terminal_restart: () => term({ startedAt: 2000 }),
  });
  render(TerminalPage);

  expect(await screen.findByText("PowerShell exited with code 1.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /uninote\s*PowerShell\s*Exited/ })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Restart" }));

  expect(calls.calls("terminal_restart")[0]).toEqual({ id: "t1" });
  expect(await screen.findByText(/Ctrl\+V pastes/)).toBeInTheDocument();
  expect(screen.queryByText("PowerShell exited with code 1.")).not.toBeInTheDocument();
});

test("a shell that exits while the screen is open is shown as exited", async () => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  vi.mocked(listen).mockImplementation(async (name, handler) => {
    handlers[name] = handler as (e: { payload: unknown }) => void;
    return () => undefined;
  });
  backend({ terminal_list: () => [term()] });
  render(TerminalPage);

  await screen.findByRole("button", { name: /^uninote/ });
  handlers["terminal-changed"]({ payload: term({ running: false, pid: null, exitCode: 0 }) });
  expect(await screen.findByText("PowerShell exited with code 0.")).toBeInTheDocument();
});

test("a list that fails to load can be tried again", async () => {
  const user = userEvent.setup();
  let fail = true;
  backend({ terminal_list: () => (fail ? new Error("boom") : []) });
  render(TerminalPage);

  expect(await screen.findByRole("heading", { name: "Terminals could not be loaded" })).toBeInTheDocument();
  fail = false;
  await user.click(screen.getByRole("button", { name: "Try again" }));
  expect(await screen.findByRole("heading", { name: "No terminal open" })).toBeInTheDocument();
});
