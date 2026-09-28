export type Platform = "windows" | "macos" | "linux";

/** WebView2 says "Windows NT", WKWebView "Macintosh", WebKitGTK "X11; Linux". */
export function platformOf(ua: string): Platform {
  if (ua.includes("Windows")) return "windows";
  if (ua.includes("Macintosh") || ua.includes("Mac OS X")) return "macos";
  return "linux";
}

/** Read on every call, so a test can stub the user agent before it renders. */
export function currentPlatform(): Platform {
  return platformOf(typeof navigator === "undefined" ? "" : navigator.userAgent);
}

/** The key that pastes into a terminal: Linux terminals keep Ctrl+V for the program inside. */
export function pasteKey(p: Platform = currentPlatform()): string {
  return p === "macos" ? "⌘V" : p === "linux" ? "Ctrl+Shift+V" : "Ctrl+V";
}

/** A project folder written the way this OS writes paths, for input placeholders. */
export function folderExample(p: Platform = currentPlatform()): string {
  return p === "windows" ? String.raw`C:\Users\you\Project\my-app` : p === "macos" ? "/Users/you/Projects/my-app" : "/home/you/projects/my-app";
}
