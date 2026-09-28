import { expect, test } from "vitest";
import { shortPath } from "./format";

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
