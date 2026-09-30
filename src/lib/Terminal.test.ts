import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { render } from "@testing-library/svelte";
import type { ILinkProviderOptions } from "@xterm/addon-web-links";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { backend } from "../test/fixtures";
import { resetHistory } from "./command-history";
import Terminal from "./Terminal.svelte";

const SAVED = String.raw`C:\Temp\opencompanion-paste\pasted-1.png`;
const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
const LINUX_UA = "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
const MAC_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)";

// jsdom has no layout, so xterm cannot tell which link is under the mouse; the tests call the addon's callbacks.
const links = vi.hoisted(() => ({
  handler: undefined as ((e: MouseEvent, uri: string) => void) | undefined,
  options: undefined as ILinkProviderOptions | undefined,
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: class {
    constructor(handler: (e: MouseEvent, uri: string) => void, options: ILinkProviderOptions) {
      links.handler = handler;
      links.options = options;
    }
    activate() {}
    dispose() {}
  },
}));

beforeEach(() => {
  vi.mocked(listen).mockImplementation(async () => () => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

function mount(kind: "session" | "terminal", screen = "") {
  const calls = backend({
    session_output: () => ({ data: screen, seq: 1 }),
    terminal_output: () => ({ data: screen, seq: 1 }),
    save_pasted_image: () => SAVED,
  });
  const { container } = render(Terminal, { id: "t1", live: true, label: "Terminal", kind });
  const input = container.querySelector("textarea") as HTMLTextAreaElement;
  const sent = () => (kind === "session" ? calls.calls("send_input").map((a) => a.text) : calls.calls("terminal_write").map((a) => a.data));
  return { container, input, sent };
}

function key(target: HTMLElement, init: KeyboardEventInit & { keyCode?: number }) {
  const ev = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(ev);
  return ev;
}

function paste(target: HTMLElement, text: string, files: File[] = []) {
  const ev = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(ev, "clipboardData", { value: { getData: () => text, files } });
  target.dispatchEvent(ev);
  return ev;
}

const tick = () => new Promise((r) => setTimeout(r, 20));

test("Shift+Enter in a session sends Alt+Enter, which the AI CLIs read as a new line", async () => {
  const { input, sent } = mount("session");
  key(input, { key: "Enter", keyCode: 13, shiftKey: true });
  await vi.waitFor(() => expect(sent()).toEqual(["\x1b\r"]));
});

test("Shift+Enter in a shell sends the real key once ConPTY asks for win32-input-mode, and Enter before that", async () => {
  const plain = mount("terminal", "PS C:\\> ");
  await vi.waitFor(() => expect(plain.container.textContent).toContain("PS C:"));
  key(plain.input, { key: "Enter", keyCode: 13, shiftKey: true });
  await vi.waitFor(() => expect(plain.sent()).toEqual(["\r"]));
  plain.container.remove();

  const win32 = mount("terminal", "\x1b[?9001hPS C:\\> ");
  await vi.waitFor(() => expect(win32.container.textContent).toContain("PS C:"));
  key(win32.input, { key: "Enter", keyCode: 13, shiftKey: true });
  await vi.waitFor(() => expect(win32.sent()).toEqual(["\x1b[13;28;13;1;16;1_\x1b[13;28;13;0;16;1_"]));
});

test("on Windows Ctrl+V in a session is left to the browser's paste, elsewhere it still reaches the CLI", async () => {
  const other = mount("session");
  key(other.input, { key: "v", keyCode: 86, ctrlKey: true });
  await vi.waitFor(() => expect(other.sent()).toEqual(["\x16"]));
  other.container.remove();

  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(WINDOWS_UA);
  const windows = mount("session");
  key(windows.input, { key: "v", keyCode: 86, ctrlKey: true });
  await tick();
  expect(windows.sent()).toEqual([]);
});

test("on Linux Ctrl+Shift+V in a session is left to the browser's paste, as in Linux terminals, and Ctrl+V reaches the CLI", async () => {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(LINUX_UA);
  const linux = mount("session");
  // The browser pastes only when the terminal leaves the key alone.
  const chord = key(linux.input, { key: "V", keyCode: 86, ctrlKey: true, shiftKey: true });
  expect(chord.defaultPrevented).toBe(false);
  await tick();
  expect(linux.sent()).toEqual([]);
  key(linux.input, { key: "v", keyCode: 86, ctrlKey: true });
  await vi.waitFor(() => expect(linux.sent()).toEqual(["\x16"]));
});

test("a pasted screenshot is saved to a file and the file's path is pasted", async () => {
  const { input, sent } = mount("session");
  const image = new File([new Uint8Array([137, 80, 78, 71])], "image.png", { type: "image/png" });
  const ev = paste(input, "", [image]);
  expect(ev.defaultPrevented).toBe(true);
  await vi.waitFor(() => expect(sent()).toEqual([SAVED]));
  const save = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "save_pasted_image");
  expect(save).toEqual(["save_pasted_image", new Uint8Array([137, 80, 78, 71]), { headers: { "x-image-type": "image/png" } }]);
});

test("pasted text still goes straight to the terminal", async () => {
  const { input, sent } = mount("terminal");
  paste(input, "npm run dev");
  await vi.waitFor(() => expect(sent()).toEqual(["npm run dev"]));
  expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "save_pasted_image")).toBe(false);
});

const LINK = "http://localhost:5173/";
const RANGE = { start: { x: 1, y: 1 }, end: { x: 22, y: 1 } };

test("a link opens in the browser on Ctrl+click, and a plain or right click leaves it alone", async () => {
  vi.mocked(openUrl).mockClear();
  const { container } = mount("terminal");
  const click = (init: MouseEventInit) => links.handler?.(new MouseEvent("mouseup", init), LINK);
  click({ button: 0 });
  click({ button: 2, ctrlKey: true });
  expect(openUrl).not.toHaveBeenCalled();
  click({ button: 0, ctrlKey: true });
  expect(openUrl).toHaveBeenCalledExactlyOnceWith(LINK);

  const host = container.querySelector(".xterm-host");
  links.options?.hover?.(new MouseEvent("mousemove"), LINK, RANGE);
  expect(host).toHaveAttribute("title", "Ctrl+click to open this link");
  links.options?.leave?.(new MouseEvent("mouseleave"), LINK);
  expect(host).not.toHaveAttribute("title");
});

test("on macOS a link opens on Cmd+click, which its tooltip names", async () => {
  vi.mocked(openUrl).mockClear();
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(MAC_UA);
  const { container } = mount("session");
  links.handler?.(new MouseEvent("mouseup", { button: 0, ctrlKey: true }), LINK);
  expect(openUrl).not.toHaveBeenCalled();
  links.handler?.(new MouseEvent("mouseup", { button: 0, metaKey: true }), LINK);
  expect(openUrl).toHaveBeenCalledExactlyOnceWith(LINK);
  links.options?.hover?.(new MouseEvent("mousemove"), LINK, RANGE);
  expect(container.querySelector(".xterm-host")).toHaveAttribute("title", "⌘+click to open this link");
});

test("a closed terminal sends nothing; a typed key wakes it once, and cursor keys or Ctrl+C do not", async () => {
  const calls = backend({ session_output: () => ({ data: "", seq: 1 }) });
  let done: () => void = () => undefined;
  const onwake = vi.fn(() => new Promise<void>((r) => (done = r)));
  const { container } = render(Terminal, { id: "t1", live: false, label: "Terminal", kind: "session", onwake });
  const input = container.querySelector("textarea") as HTMLTextAreaElement;

  key(input, { key: "ArrowUp", keyCode: 38 });
  key(input, { key: "c", keyCode: 67, ctrlKey: true });
  await tick();
  expect(onwake).not.toHaveBeenCalled();

  key(input, { key: "Enter", keyCode: 13 });
  key(input, { key: "Enter", keyCode: 13 });
  await vi.waitFor(() => expect(onwake).toHaveBeenCalledTimes(1));
  expect(onwake).toHaveBeenCalledWith(expect.any(Number), expect.any(Number));
  expect(calls.calls("send_input")).toEqual([]);

  // Once that start settles and the terminal is still closed, the next key tries again.
  done();
  await tick();
  key(input, { key: "Enter", keyCode: 13 });
  await vi.waitFor(() => expect(onwake).toHaveBeenCalledTimes(2));
});

test("a shell terminal closes without reading its id again, which its parent may have dropped by then", async () => {
  backend({ terminal_output: () => ({ data: "", seq: 1 }) });
  // The Session screen passes `id={x.id}`; once the shell leaves its list, `x` is null and reading the prop throws.
  let gone = false;
  let lateReads = 0;
  const props = {
    get id() {
      if (!gone) return "s1";
      lateReads++;
      throw new TypeError("null is not an object (evaluating 'x.id')");
    },
    live: true,
    label: "Terminal",
    kind: "terminal" as const,
  };
  const { container, unmount } = render(Terminal, props);
  await vi.waitFor(() => expect(container.querySelector("textarea")).not.toBeNull());
  gone = true;
  unmount();
  await tick();
  expect(lateReads).toBe(0);
});

test("a program's copy through OSC 52 reaches the clipboard, as Claude Code copies over SSH, and a replayed one does not", async () => {
  let output: ((e: { payload: { id: string; data: string; seq: number } }) => void) | undefined;
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    output = handler as typeof output;
    return () => undefined;
  });
  const copy = (text: string) => `\x1b]52;c;${btoa(text)}\x07`;
  // The screen shown on opening holds a copy made before; writing it again would replace the clipboard.
  const calls = backend({ terminal_output: () => ({ data: `${copy("copied an hour ago")}user@box:~$ `, seq: 1 }) });
  const { container } = render(Terminal, { id: "t1", live: true, label: "Terminal", kind: "terminal" });
  await vi.waitFor(() => expect(container.textContent).toContain("user@box"));
  await tick();
  expect(calls.calls("write_clipboard")).toEqual([]);

  output?.({ payload: { id: "t1", data: copy("claude --resume"), seq: 2 } });
  await vi.waitFor(() => expect(calls.calls("write_clipboard")).toEqual([{ text: "claude --resume" }]));

  // A burst of copies in one chunk keeps only the last, the one the clipboard would end up holding.
  output?.({ payload: { id: "t1", data: copy("one") + copy("two") + copy("three"), seq: 3 } });
  await vi.waitFor(() => expect(calls.calls("write_clipboard")).toHaveLength(2));
  await tick();
  expect(calls.calls("write_clipboard")).toEqual([{ text: "claude --resume" }, { text: "three" }]);
});

/** A shell tab in `C:\app` whose shell echoes through `echo`, with the history the backend sends for the folder. */
function shellTab(screen: string) {
  resetHistory();
  let output: ((e: { payload: { id: string; data: string; seq: number } }) => void) | undefined;
  vi.mocked(listen).mockImplementation(async (event, handler) => {
    if (event === "terminal-output") output = handler as typeof output;
    return () => undefined;
  });
  const calls = backend({
    terminal_output: () => ({ data: screen, seq: 1 }),
    shell_history: () => ({ here: ["git status"], elsewhere: ["git switch main"], imported: ["git stash"] }),
  });
  const { container } = render(Terminal, { id: "t1", live: true, label: "Terminal", kind: "terminal", folder: String.raw`C:\app` });
  const input = container.querySelector("textarea") as HTMLTextAreaElement;
  let seq = 1;
  const echo = (data: string) => output?.({ payload: { id: "t1", data, seq: ++seq } });
  const sent = () => calls.calls("terminal_write").map((a) => a.data);
  return { container, input, echo, sent, calls };
}

const PROMPT = "PS C:\\app> \x1b]133;B\x07";

test("a shell tab suggests the last matching command after its prompt, and Right Arrow types the rest", async () => {
  const { container, input, echo, sent, calls } = shellTab(PROMPT);
  await vi.waitFor(() => expect(calls.calls("shell_history")).toEqual([{ folder: String.raw`C:\app` }]));
  echo("git st");
  await vi.waitFor(() => expect(container.querySelector(".suggestion")?.textContent).toBe("atus"));
  expect(container.querySelector(".suggestion")).toHaveAttribute("aria-hidden", "true");
  // Screen readers hear it once it has stayed a moment.
  await vi.waitFor(() => expect(container.querySelector("p.sr-only[aria-live]")).toHaveTextContent("Suggested command: git status. Right Arrow accepts it."));

  key(input, { key: "ArrowRight", keyCode: 39 });
  await vi.waitFor(() => expect(sent()).toEqual(["atus"]));
  expect(container.querySelector(".suggestion")).toBeNull();
});

test("a key typed hides the suggestion until the shell echoes it, so Right Arrow never adds an older one's rest", async () => {
  const { container, input, echo, sent } = shellTab(PROMPT);
  await tick();
  echo("git s");
  await vi.waitFor(() => expect(container.querySelector(".suggestion")?.textContent).toBe("tatus"));
  key(input, { key: "w", keyCode: 87 });
  expect(container.querySelector(".suggestion")).toBeNull();
  key(input, { key: "ArrowRight", keyCode: 39 });
  await vi.waitFor(() => expect(sent()).toEqual(["w", "\x1b[C"]));
  // Once the echo arrives the match comes from another folder's commands.
  echo("w");
  await vi.waitFor(() => expect(container.querySelector(".suggestion")?.textContent).toBe("itch main"));
  // Enter runs the line: no suggestion until the next prompt.
  key(input, { key: "Enter", keyCode: 13 });
  echo("\r\n");
  await tick();
  expect(container.querySelector(".suggestion")).toBeNull();
});

test("other output arriving before a key's echo does not bring back the suggestion for the text before that key", async () => {
  const { container, input, echo, sent } = shellTab(PROMPT);
  await tick();
  echo("g");
  await vi.waitFor(() => expect(container.querySelector(".suggestion")?.textContent).toBe("it status"));
  key(input, { key: "i", keyCode: 73 });
  // A title change lands first; the line still reads "g" while "i" is on its way.
  echo("\x1b]0;PowerShell\x07");
  await tick();
  expect(container.querySelector(".suggestion")).toBeNull();
  key(input, { key: "ArrowRight", keyCode: 39 });
  await vi.waitFor(() => expect(sent()).toEqual(["i", "\x1b[C"]));
  echo("i");
  await vi.waitFor(() => expect(container.querySelector(".suggestion")?.textContent).toBe("t status"));
});

test("a shell without the prompt mark, such as Command Prompt, gets no suggestion", async () => {
  const { container, input, echo, sent } = shellTab("C:\\app>");
  await tick();
  echo("git st");
  await tick();
  expect(container.querySelector(".suggestion")).toBeNull();
  key(input, { key: "ArrowRight", keyCode: 39 });
  await vi.waitFor(() => expect(sent()).toEqual(["\x1b[C"]));
});
