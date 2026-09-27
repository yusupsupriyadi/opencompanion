// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
    // Only the phone companion registers the service worker (routes/m), and only where the browser
    // allows one; the desktop window never does. It keeps the static files the phone screens show.
    serviceWorker: {
      register: false,
      files: (file) => file === "favicon.png" || file === "meadow-day.png" || file.startsWith("m/"),
    },
  },
};

export default config;
