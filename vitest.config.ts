import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";
import path from "node:path";
import { defineConfig } from "vitest/config";

// Component tests run the real Svelte components against a mocked Tauri backend.
// `$app/*` points at small stubs so no SvelteKit runtime is needed.
export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  resolve: {
    alias: {
      $lib: path.resolve("src/lib"),
      "$app/state": path.resolve("src/test/app-state.svelte.ts"),
      "$app/navigation": path.resolve("src/test/app-navigation.ts"),
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    include: ["src/**/*.test.ts"],
  },
});
