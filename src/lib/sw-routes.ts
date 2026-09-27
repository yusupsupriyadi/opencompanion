// Which requests the phone's service worker (src/service-worker.ts) may answer from its cache.
// Only the app itself is kept: API calls, the live connection, pairing and anything that carries
// a device token always go to the desktop, so no token or pairing answer is ever stored.

/** The one cache key for the page. Every `/m` address gets the same SPA shell, so a pairing code
 * in `/m/pair?code=…` never becomes part of what is stored. */
export const SHELL = "/m";

/** `shell`: the desktop's page when it answers, else the kept one. `cached`: a built or static
 * file, kept since install. `null`: the browser handles it, the service worker stays out. */
export type Strategy = "shell" | "cached" | null;

export function inScope(pathname: string) {
  return pathname === "/m" || pathname.startsWith("/m/");
}

export function strategy(
  request: Pick<Request, "method" | "url" | "mode" | "headers">,
  origin: string,
  kept: ReadonlySet<string>,
): Strategy {
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== origin) return null;
  if (url.pathname.startsWith("/api/") || request.headers.has("Authorization")) return null;
  if (request.mode === "navigate") return inScope(url.pathname) ? "shell" : null;
  return kept.has(url.pathname) ? "cached" : null;
}
