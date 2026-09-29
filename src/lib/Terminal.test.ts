import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { render } from "@testing-library/svelte";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { backend } from "../test/fixtures";
import Terminal from "./Terminal.svelte";

const SAVED = String.raw`C:\Temp\opencompanion-paste\pasted-1.png`;
const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
const LINUX_UA = "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

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
