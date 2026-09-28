import { expect, test } from "vitest";
import { parseDiff } from "./diff";

test("a diff becomes hunks and numbered lines, without the header", () => {
  const patch = [
    "diff --git a/src/app.ts b/src/app.ts",
    "index 1111111..2222222 100644",
    "--- a/src/app.ts",
    "+++ b/src/app.ts",
    "@@ -3,3 +3,4 @@ function main() {",
    " const a = 1;",
    "-const b = 2;",
    "+const b = 3;",
    "+const c = 4;",
    " return a;",
    "\\ No newline at end of file",
    "",
  ].join("\n");
  expect(parseDiff(patch)).toEqual([
    { kind: "hunk", text: "@@ -3,3 +3,4 @@ function main() {" },
    { kind: "ctx", old: 3, new: 3, text: "const a = 1;" },
    { kind: "del", old: 4, new: null, text: "const b = 2;" },
    { kind: "add", old: null, new: 4, text: "const b = 3;" },
    { kind: "add", old: null, new: 5, text: "const c = 4;" },
    { kind: "ctx", old: 5, new: 6, text: "return a;" },
    { kind: "note", text: "No newline at end of file" },
  ]);
});

test("a new file says so and counts from the first line", () => {
  const patch = "diff --git a/n.txt b/n.txt\r\nnew file mode 100644\r\nindex 0000000..8ba3a16\r\n--- /dev/null\r\n+++ b/n.txt\r\n@@ -0,0 +1 @@\r\n+fresh\r\n";
  expect(parseDiff(patch)).toEqual([
    { kind: "note", text: "new file mode 100644" },
    { kind: "hunk", text: "@@ -0,0 +1 @@" },
    { kind: "add", old: null, new: 1, text: "fresh" },
  ]);
});

test("an empty patch has no rows", () => {
  expect(parseDiff("")).toEqual([]);
});
