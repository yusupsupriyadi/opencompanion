import { expect, test } from "vitest";
import { osc52Text } from "./osc52";

const b64 = (text: string) => btoa(String.fromCharCode(...new TextEncoder().encode(text)));

test("a copy is the payload's UTF-8 text, whichever selection it names", () => {
  expect(osc52Text(`c;${b64("npm run dev")}`)).toBe("npm run dev");
  expect(osc52Text(`p;${b64("naïve ✓ 日本")}`)).toBe("naïve ✓ 日本");
  // tmux leaves the selection empty.
  expect(osc52Text(`;${b64("line 1\nline 2")}`)).toBe("line 1\nline 2");
});

test("a payload wrapped over lines is read whole", () => {
  const wrapped = b64("a longer selection that a program wraps").replace(/(.{8})/g, "$1\n");
  expect(osc52Text(`c;${wrapped}`)).toBe("a longer selection that a program wraps");
});

test("a request to read the clipboard, to clear it, or anything malformed copies nothing", () => {
  expect(osc52Text("c;?")).toBeNull();
  expect(osc52Text("c;")).toBeNull();
  expect(osc52Text(b64("no selection field"))).toBeNull();
  expect(osc52Text(`x;${b64("unknown selection")}`)).toBeNull();
  expect(osc52Text("c;not base64!")).toBeNull();
});

test("an oversized payload copies nothing", () => {
  expect(osc52Text(`c;${"A".repeat(1024 * 1024 + 4)}`)).toBeNull();
});
