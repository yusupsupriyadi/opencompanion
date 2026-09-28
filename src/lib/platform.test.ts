import { expect, test } from "vitest";
import { folderExample, pasteKey, platformOf } from "./platform";

test("each OS names its own paste key and folder example", () => {
  expect(pasteKey("windows")).toBe("Ctrl+V");
  expect(pasteKey("macos")).toBe("⌘V");
  expect(pasteKey("linux")).toBe("Ctrl+Shift+V");
  expect(folderExample("windows")).toBe(String.raw`C:\Users\you\Project\my-app`);
  expect(folderExample("macos")).toBe("/Users/you/Projects/my-app");
  expect(folderExample("linux")).toBe("/home/you/projects/my-app");
});

test("the webview's user agent names the OS", () => {
  expect(platformOf("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0")).toBe(
    "windows",
  );
  expect(platformOf("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)")).toBe("macos");
  expect(platformOf("Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15")).toBe("linux");
  // jsdom names the host's platform in lower case, which no real webview does.
  expect(platformOf("Mozilla/5.0 (win32) AppleWebKit/537.36 (KHTML, like Gecko) jsdom/27.0.0")).toBe("linux");
});
