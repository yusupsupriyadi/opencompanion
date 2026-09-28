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
