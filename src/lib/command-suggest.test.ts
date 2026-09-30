import { expect, test } from "vitest";
import { suggestion, typedText, type BufferRows } from "./command-suggest";

/**
 * A buffer as wide as its longest row, so a row that wraps fills it; a row starting with "↩" continues the one above,
 * as a wrapped row does.
 */
function rows(...lines: string[]): BufferRows {
  const texts = lines.map((l) => l.replace(/^↩/, ""));
  const cols = Math.max(...texts.map((t) => t.length));
  return {
    getLine(y) {
      const text = texts[y];
      if (text === undefined) return undefined;
      return {
        isWrapped: lines[y].startsWith("↩"),
        translateToString: (trimRight = false, start = 0, end = cols) => {
          const part = text.padEnd(cols).slice(start, end);
          return trimRight ? part.trimEnd() : part;
        },
      };
    },
  };
}

test("what was typed runs from the prompt mark to the cursor", () => {
  const buf = rows("PS C:\\app> git st");
  expect(typedText(buf, { y: 0, x: 11 }, { y: 0, x: 17 })).toBe("git st");
  // Spaces typed at the end count: "git " matches "git status" but not "gitk".
  expect(typedText(rows("$ git "), { y: 0, x: 2 }, { y: 0, x: 6 })).toBe("git ");
  expect(typedText(rows("PS C:\\app> "), { y: 0, x: 11 }, { y: 0, x: 11 })).toBe("");
});

test("a line typed past the edge is read across its wrapped rows", () => {
  const buf = rows("$ npm run build --work", "↩space=web");
  expect(typedText(buf, { y: 0, x: 2 }, { y: 1, x: 9 })).toBe("npm run build --workspace=web");
});

test("nothing is read with text after the cursor, on a row that is not a wrap, or before the mark", () => {
  // The cursor was moved back into the line.
  expect(typedText(rows("$ git status"), { y: 0, x: 2 }, { y: 0, x: 5 })).toBeNull();
  // A wrapped row continues after the cursor's row.
  expect(typedText(rows("$ npm run build", "↩ --x"), { y: 0, x: 2 }, { y: 0, x: 15 })).toBeNull();
  // A continuation prompt (PowerShell's >>) is a row of its own, not a wrap.
  expect(typedText(rows("PS> if ($x) {", ">> ls"), { y: 0, x: 4 }, { y: 1, x: 5 })).toBeNull();
  expect(typedText(rows("$ ls"), { y: 1, x: 2 }, { y: 0, x: 4 })).toBeNull();
  expect(typedText(rows("$ ls"), { y: 0, x: 3 }, { y: 0, x: 2 })).toBeNull();
  expect(typedText(rows("$ ls"), { y: 5, x: 0 }, { y: 5, x: 0 })).toBeNull();
});

test("the suggestion is the first longer match, this folder's before other folders' and the shells' own", () => {
  const here = ["bun run dev", "git status"];
  const elsewhere = ["bun run build", "gh pr list"];
  const imported = ["git stash", "gh pr view"];
  const lists = [here, elsewhere, imported];
  expect(suggestion("bun r", lists)).toBe("bun run dev");
  expect(suggestion("bun run b", lists)).toBe("bun run build");
  expect(suggestion("gh pr v", lists)).toBe("gh pr view");
  expect(suggestion("git sta", lists)).toBe("git status");
  expect(suggestion("git stas", lists)).toBe("git stash");
  // Case-sensitive, like the shells' own history search.
  expect(suggestion("Git", lists)).toBeNull();
});

test("there is no suggestion for nothing typed, a complete command or a command no list has", () => {
  const lists = [["git status"], [], []];
  expect(suggestion("", lists)).toBeNull();
  expect(suggestion("   ", lists)).toBeNull();
  expect(suggestion("git status", lists)).toBeNull();
  expect(suggestion("docker", lists)).toBeNull();
});
