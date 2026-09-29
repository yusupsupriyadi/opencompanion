// Files the viewer draws as a picture or as pages instead of showing their text.

export type Media = { kind: "image" | "pdf"; mime: string; label: string };

// Only formats every webview the app runs in (WebView2, WKWebView, WebKitGTK) can draw.
const BY_EXT = new Map<string, Media>([
  ["png", { kind: "image", mime: "image/png", label: "PNG" }],
  ["jpg", { kind: "image", mime: "image/jpeg", label: "JPEG" }],
  ["jpeg", { kind: "image", mime: "image/jpeg", label: "JPEG" }],
  ["gif", { kind: "image", mime: "image/gif", label: "GIF" }],
  ["webp", { kind: "image", mime: "image/webp", label: "WebP" }],
  ["avif", { kind: "image", mime: "image/avif", label: "AVIF" }],
  ["bmp", { kind: "image", mime: "image/bmp", label: "BMP" }],
  ["ico", { kind: "image", mime: "image/x-icon", label: "ICO" }],
  ["svg", { kind: "image", mime: "image/svg+xml", label: "SVG" }],
  ["pdf", { kind: "pdf", mime: "application/pdf", label: "PDF" }],
]);

export function mediaFor(path: string): Media | null {
  const name = (path.split(/[\\/]/).pop() ?? "").toLowerCase();
  const dot = name.lastIndexOf(".");
  return dot > 0 ? (BY_EXT.get(name.slice(dot + 1)) ?? null) : null;
}

/** Zoom levels the viewer steps through; 1 is the file's own size. */
export const ZOOM_STEPS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];

/** The next step up (`dir` 1) or down (-1) from `now`, or null past the last one. */
export function zoomStep(now: number, dir: 1 | -1): number | null {
  const next = dir > 0 ? ZOOM_STEPS.find((z) => z > now + 0.001) : ZOOM_STEPS.findLast((z) => z < now - 0.001);
  return next ?? null;
}
