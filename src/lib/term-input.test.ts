import { expect, test } from "vitest";
import { pastedImage, pathForPaste, shiftEnter } from "./term-input";

const png = new File([new Uint8Array([137, 80, 78, 71])], "image.png", { type: "image/png" });
const svg = new File(["<svg/>"], "logo.svg", { type: "image/svg+xml" });
const clip = (text: string, files: File[]) => ({ getData: () => text, files: files as unknown as FileList });

test("a paste without text hands over its image, and text wins when there is both", () => {
  expect(pastedImage(clip("", [png]))).toBe(png);
  expect(pastedImage(clip("npm run dev", [png]))).toBeNull();
  expect(pastedImage(clip("", [svg]))).toBeNull();
  expect(pastedImage(clip("", []))).toBeNull();
  expect(pastedImage(null)).toBeNull();
});

test("a pasted path is quoted only when it holds a space", () => {
  expect(pathForPaste(String.raw`C:\Temp\pasted-1.png`)).toBe(String.raw`C:\Temp\pasted-1.png`);
  expect(pathForPaste(String.raw`C:\Users\Ada Lovelace\Temp\pasted-1.png`)).toBe(String.raw`"C:\Users\Ada Lovelace\Temp\pasted-1.png"`);
});

test("Shift+Enter is Alt+Enter for a CLI, and the real key for a shell only in win32-input-mode", () => {
  expect(shiftEnter("session", false)).toBe("\x1b\r");
  expect(shiftEnter("terminal", true)).toBe("\x1b[13;28;13;1;16;1_\x1b[13;28;13;0;16;1_");
  expect(shiftEnter("terminal", false)).toBeNull();
});
