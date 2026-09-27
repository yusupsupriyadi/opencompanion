import { expect, test } from "vitest";
import { parseTail } from "./tail";

test("plain lines stay as they are, with blank runs folded to one line", () => {
  expect(parseTail("\n\nnpm test\n\n\n\n  12 passed\n\n")).toEqual([{ kind: "text", text: "npm test\n\n  12 passed" }]);
});

test("a TUI's input box becomes a framed box, its title kept and its padding dropped", () => {
  const screen = [
    "● Done, the tests pass.",
    "╭─── Claude Code ─────────────────────────────╮",
    "│                                             │",
    "│ > add a test for the login bug              │",
    "├─────────────────────────────────────────────┤",
    "│ ? for shortcuts                             │",
    "╰─────────────────────────────────────────────╯",
    "  auto-accept edits on",
  ].join("\n");
  expect(parseTail(screen)).toEqual([
    { kind: "text", text: "● Done, the tests pass." },
    { kind: "box", title: "Claude Code", lines: ["> add a test for the login bug", null, "? for shortcuts"] },
    { kind: "text", text: "  auto-accept edits on" },
  ]);
});

test("a full-width rule is drawn by the page instead of wrapping into rows of dashes", () => {
  expect(parseTail("Plan\n────────────────────────────────\n1. Read the code")).toEqual([
    { kind: "text", text: "Plan" },
    { kind: "rule" },
    { kind: "text", text: "1. Read the code" },
  ]);
});

test("a box the screen cut off keeps its rows, and the line after it reads on its own", () => {
  expect(parseTail("╭──────────╮\n│ > hi     │\nplain text")).toEqual([
    { kind: "box", title: "", lines: ["> hi"] },
    { kind: "text", text: "plain text" },
  ]);
});

test("an empty screen has no blocks", () => {
  expect(parseTail("")).toEqual([]);
  expect(parseTail("\n   \n")).toEqual([]);
});
