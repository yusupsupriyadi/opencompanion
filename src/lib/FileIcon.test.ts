import File from "phosphor-svelte/lib/File";
import FileCode from "phosphor-svelte/lib/FileCode";
import FileImage from "phosphor-svelte/lib/FileImage";
import FileIni from "phosphor-svelte/lib/FileIni";
import FileLock from "phosphor-svelte/lib/FileLock";
import FileMd from "phosphor-svelte/lib/FileMd";
import FilePdf from "phosphor-svelte/lib/FilePdf";
import FilePng from "phosphor-svelte/lib/FilePng";
import FileTs from "phosphor-svelte/lib/FileTs";
import { render } from "@testing-library/svelte";
import { expect, test } from "vitest";
import FileIcon, { fileIcon, fileTone } from "./FileIcon.svelte";

test("a file's glyph follows its type, in any case, and falls back to a plain page", () => {
  expect(fileIcon("docs/spec.pdf")).toBe(FilePdf);
  expect(fileIcon(String.raw`C:\repo\README.MD`)).toBe(FileMd);
  expect(fileIcon("design/logo.png")).toBe(FilePng);
  expect(fileIcon("shot.webp")).toBe(FileImage);
  expect(fileIcon("src/app.ts")).toBe(FileTs);
  expect(fileIcon("src/App.svelte")).toBe(FileCode);
  expect(fileIcon("Cargo.lock")).toBe(FileLock);
  expect(fileIcon("package-lock.json")).toBe(FileLock);
  expect(fileIcon(".env.local")).toBe(FileIni);
  expect(fileIcon("LICENSE")).toBe(File);
  expect(fileIcon(".png")).toBe(File);
});

test("a glyph takes the color people tie to its type, and plain files keep the tab's color", () => {
  expect(fileTone("spec.pdf")).toBe("red");
  expect(fileTone("README.md")).toBe("blue");
  expect(fileTone("src/app.ts")).toBe("blue");
  expect(fileTone("src/main.js")).toBe("yellow");
  expect(fileTone("package.json")).toBe("yellow");
  expect(fileTone(".env.local")).toBe("yellow");
  expect(fileTone("logo.PNG")).toBe("purple");
  expect(fileTone("src-tauri/src/lib.rs")).toBe("orange");
  expect(fileTone("App.vue")).toBe("green");
  expect(fileTone("Dockerfile")).toBe("blue");
  expect(fileTone("notes.txt")).toBeNull();
  expect(fileTone("package-lock.json")).toBeNull();
  expect(fileTone("LICENSE")).toBeNull();
});

test("the glyph is decoration, hidden from screen readers, and carries its color class", () => {
  const { container } = render(FileIcon, { path: "spec.pdf", size: 18 });
  const svg = container.querySelector("svg");
  expect(svg).toHaveAttribute("aria-hidden", "true");
  expect(svg).toHaveAttribute("width", "18");
  expect(svg).toHaveClass("ft-red");
  const plain = render(FileIcon, { path: "LICENSE" }).container.querySelector("svg");
  expect(plain?.getAttribute("class") ?? "").not.toMatch(/ft-/);
});
