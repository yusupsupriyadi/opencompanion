import { expect, test } from "vitest";
import { SHELL, strategy } from "./sw-routes";

const ORIGIN = "http://192.168.1.24:8765";
const KEPT = new Set(["/_app/immutable/entry/start.js", "/m/icons/icon-192.png", "/meadow-day.png"]);

function req(path: string, over: { method?: string; mode?: RequestMode; headers?: Record<string, string>; origin?: string } = {}) {
  return {
    url: `${over.origin ?? ORIGIN}${path}`,
    method: over.method ?? "GET",
    mode: over.mode ?? "cors",
    headers: new Headers(over.headers),
  };
}

test("the API, the live connection and pairing always go to the desktop", () => {
  expect(strategy(req("/api/sessions"), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/api/ws?token=device-token"), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/api/pair", { method: "POST" }), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/api/hello"), ORIGIN, KEPT)).toBeNull();
  // Anything carrying a device token, wherever it points.
  expect(strategy(req("/m/icons/icon-192.png", { headers: { Authorization: "Bearer device-token" } }), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/m/icons/icon-192.png", { method: "POST" }), ORIGIN, KEPT)).toBeNull();
});

test("phone pages get the shell, stored under one key, so a pairing code is never kept", () => {
  expect(SHELL).toBe("/m");
  expect(strategy(req("/m", { mode: "navigate" }), ORIGIN, KEPT)).toBe("shell");
  expect(strategy(req("/m/pair?code=482913", { mode: "navigate" }), ORIGIN, KEPT)).toBe("shell");
  expect(strategy(req("/m/session?id=s1", { mode: "navigate" }), ORIGIN, KEPT)).toBe("shell");
  // Desktop screens and look-alike paths are not the phone's.
  expect(strategy(req("/settings", { mode: "navigate" }), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/mail", { mode: "navigate" }), ORIGIN, KEPT)).toBeNull();
});

test("only the built and static files from install are answered from the cache", () => {
  expect(strategy(req("/_app/immutable/entry/start.js"), ORIGIN, KEPT)).toBe("cached");
  expect(strategy(req("/meadow-day.png"), ORIGIN, KEPT)).toBe("cached");
  expect(strategy(req("/_app/version.json"), ORIGIN, KEPT)).toBeNull();
  expect(strategy(req("/_app/immutable/entry/start.js", { origin: "https://cdn.example" }), ORIGIN, KEPT)).toBeNull();
});
