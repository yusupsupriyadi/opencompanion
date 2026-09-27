import { readFileSync } from "node:fs";
import path from "node:path";
import { render } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { beforeEach, expect, test, vi } from "vitest";
import { setUrl } from "../../test/app-state.svelte";
import Layout from "./+layout.svelte";

type Icon = { src: string; sizes: string; type: string; purpose: string };
const manifest = JSON.parse(readFileSync(path.resolve("static/m/manifest.webmanifest"), "utf8")) as {
  start_url: string;
  scope: string;
  display: string;
  icons: Icon[];
};

/** Width x height from a PNG's header. */
function pngSize(src: string) {
  const bytes = readFileSync(path.resolve("static", src.replace(/^\//, "")));
  return `${bytes.readUInt32BE(16)}x${bytes.readUInt32BE(20)}`;
}

beforeEach(() => {
  localStorage.clear();
  setUrl("/m/pair");
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ app: "OpenCompanion", language: "en" }))));
});

test("the manifest opens the phone app without browser bars, inside /m, with icons at their stated size", () => {
  expect(manifest).toMatchObject({ start_url: "/m", scope: "/m", display: "standalone" });
  expect(manifest.icons.map((i) => `${i.sizes} ${i.purpose}`)).toEqual(
    expect.arrayContaining(["192x192 any", "512x512 any", "192x192 maskable", "512x512 maskable"]),
  );
  for (const icon of manifest.icons) expect(pngSize(icon.src), icon.src).toBe(icon.sizes);
  expect(pngSize("/m/icons/apple-touch-icon.png")).toBe("180x180");
});

test("phone pages carry the install tags and reach the screen edges; leaving /m puts the viewport back", () => {
  const viewport = Object.assign(document.createElement("meta"), { name: "viewport", content: "width=device-width, initial-scale=1" });
  document.head.append(viewport);
  const children = createRawSnippet(() => ({ render: () => "<main>screen</main>" }));
  const { unmount } = render(Layout, { props: { children } });

  expect(document.head.querySelector('link[rel="manifest"]')).toHaveAttribute("href", "/m/manifest.webmanifest");
  expect(document.head.querySelector('link[rel="apple-touch-icon"]')).toHaveAttribute("href", "/m/icons/apple-touch-icon.png");
  expect(document.head.querySelector('meta[name="apple-mobile-web-app-capable"]')).toHaveAttribute("content", "yes");
  expect(document.head.querySelector('meta[name="apple-mobile-web-app-status-bar-style"]')).toHaveAttribute("content", "default");
  expect(document.head.querySelectorAll('meta[name="theme-color"]')).toHaveLength(2);
  expect(viewport.content).toBe("width=device-width, initial-scale=1, viewport-fit=cover");

  unmount();
  expect(viewport.content).toBe("width=device-width, initial-scale=1");
  expect(document.head.querySelector('link[rel="manifest"]')).toBeNull();
  viewport.remove();
});
