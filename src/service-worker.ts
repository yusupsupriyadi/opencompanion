/// <reference types="@sveltejs/kit" />
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
// The phone companion's service worker. routes/m registers it with scope `/m`, and only in a secure
// context (HTTPS or localhost; the plain-HTTP LAN address is not one). It keeps the app shell and
// the built files, so the installed app opens while the desktop is away and shows its offline
// screen; what it may answer is decided in lib/sw-routes.ts.
import { build, files, version } from "$service-worker";
import { SHELL, strategy } from "./lib/sw-routes";

const sw = self as unknown as ServiceWorkerGlobalScope;
const CACHE = `phone-${version}`;
// Every font ships as woff2 and woff; phone browsers take the woff2, so the woff copies stay out.
const ASSETS = [...build.filter((file) => !file.endsWith(".woff")), ...files];
const kept = new Set(ASSETS);

sw.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll([SHELL, ...ASSETS])));
});

// A new build brings its own cache; the older ones go once it takes over.
sw.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)))),
  );
});

sw.addEventListener("fetch", (event) => {
  const how = strategy(event.request, sw.location.origin, kept);
  if (how === "cached") event.respondWith(caches.match(event.request).then((hit) => hit ?? fetch(event.request)));
  if (how === "shell") event.respondWith(shell(event.request));
});

/** Network first, so a newer desktop build shows up at once; the kept shell when nothing answers. */
async function shell(request: Request) {
  const cache = await caches.open(CACHE);
  try {
    const res = await fetch(request);
    if (res.ok && res.headers.get("Content-Type")?.startsWith("text/html")) await cache.put(SHELL, res.clone());
    return res;
  } catch {
    return (await cache.match(SHELL)) ?? Response.error();
  }
}
