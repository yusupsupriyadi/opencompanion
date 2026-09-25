import { vi } from "vitest";
import { setUrl } from "./app-state.svelte";

// Stand-in for SvelteKit's `$app/navigation`: records where the app wanted to go.
export const goto = vi.fn(async (href: string) => {
  setUrl(href);
});
