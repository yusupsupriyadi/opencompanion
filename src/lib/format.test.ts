import { expect, test } from "vitest";
import { session } from "../test/fixtures";
import { shortPath, waitingTitle } from "./format";

test("a question from OpenCode or Codex reads as waiting, not as a permission", () => {
  const asking = (reason: string) =>
    waitingTitle(session({ cli: "opencode", status: "waiting", waiting: { reason, tool: null, detail: "Which database?", requestId: null, canAnswer: false, method: "plugin", since: 0 } }));
  expect(asking("question")).toBe("OpenCode is waiting for you");
  expect(asking("permission")).toBe("OpenCode needs your permission");
});

test("the home folder shows as ~ on Windows, macOS and Linux", () => {
  expect(shortPath(String.raw`C:\Users\ana\Project\app`)).toBe("~/Project/app");
  expect(shortPath("/Users/ana/Projects/app")).toBe("~/Projects/app");
  expect(shortPath("/home/ana/app")).toBe("~/app");
  expect(shortPath("/home/ana")).toBe("~");
});

test("paths outside a home folder stay as they are", () => {
  expect(shortPath("/opt/app")).toBe("/opt/app");
  expect(shortPath("/homework/app")).toBe("/homework/app");
  expect(shortPath(String.raw`D:\work\app`)).toBe(String.raw`D:\work\app`);
});
