import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { phoneServer } from "../test/phone-server";

/** A WebSocket the test opens and closes by hand. */
class FakeSocket {
  static OPEN = 1;
  static all: FakeSocket[] = [];
  readyState = 0;
  onopen: (() => void) | null = null;
  onclose: ((e: { code: number }) => void) | null = null;
  onmessage: ((e: { data: string }) => void) | null = null;
  constructor(public url: string) {
    FakeSocket.all.push(this);
  }
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  close() {
    this.readyState = 3;
  }
  drop(code: number) {
    this.readyState = 3;
    this.onclose?.({ code });
  }
}

// Each test gets the module fresh, with no socket or retry left from the one before.
async function client() {
  vi.resetModules();
  const mod = await import("./phone.svelte");
  mod.setToken("device-token");
  return mod;
}

beforeEach(() => {
  FakeSocket.all = [];
  vi.stubGlobal("WebSocket", FakeSocket);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  localStorage.clear();
});

test("the service worker is registered for /m only in a secure context, and never in the desktop window", async () => {
  const { registerWorker } = await client();
  const register = vi.fn(async () => ({}));
  Object.defineProperty(navigator, "serviceWorker", { value: { register }, configurable: true });
  try {
    // The LAN address over plain HTTP.
    vi.stubGlobal("isSecureContext", false);
    registerWorker();
    vi.stubGlobal("isSecureContext", true);
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    registerWorker();
    expect(register).not.toHaveBeenCalled();

    vi.unstubAllGlobals();
    vi.stubGlobal("isSecureContext", true);
    registerWorker();
    expect(register).toHaveBeenCalledWith("/service-worker.js", expect.objectContaining({ scope: "/m" }));
  } finally {
    delete (navigator as { serviceWorker?: unknown }).serviceWorker;
  }
});

test("a phone removed on the desktop goes back to pairing when its connection closes", async () => {
  phoneServer({ "GET /api/sessions": () => [200, { sessions: [] }] });
  const { connect, getToken, phone } = await client();
  connect();
  FakeSocket.all[0].open();
  expect(phone.connection).toBe("online");

  FakeSocket.all[0].drop(4401);
  expect(phone.unpaired).toBe(true);
  expect(getToken()).toBeNull();
  await vi.advanceTimersByTimeAsync(30_000);
  expect(FakeSocket.all).toHaveLength(1);
});

test("one failed request while the live connection is up does not show the offline screen", async () => {
  phoneServer({ "GET /api/sessions": () => [200, { sessions: [] }] });
  const { call, connect, phone } = await client();
  connect();
  FakeSocket.all[0].open();
  vi.stubGlobal("fetch", vi.fn(async () => Promise.reject(new TypeError("Failed to fetch"))));

  await expect(call("/api/chat")).rejects.toThrow("Can't reach your desktop.");
  expect(phone.connection).toBe("online");
});

test("when the desktop goes away the phone keeps trying by itself and comes back", async () => {
  let up = false;
  const fetchMock = vi.fn(async () => {
    if (!up) throw new TypeError("Failed to fetch");
    return new Response(JSON.stringify({ sessions: [] }), { status: 200 });
  });
  vi.stubGlobal("fetch", fetchMock);
  const { connect, phone } = await client();
  connect();
  FakeSocket.all[0].open();
  FakeSocket.all[0].drop(1006);
  expect(phone.connection).toBe("offline");
  fetchMock.mockClear();

  // Still away at the first two tries; they back off rather than hammer the network.
  await vi.advanceTimersByTimeAsync(3000);
  await vi.advanceTimersByTimeAsync(6000);
  expect(fetchMock).toHaveBeenCalledTimes(2);
  expect(FakeSocket.all).toHaveLength(1);

  up = true;
  await vi.advanceTimersByTimeAsync(10_000);
  expect(FakeSocket.all).toHaveLength(2);
  FakeSocket.all[1].open();
  expect(phone.connection).toBe("online");

  // A late close from the old socket does not knock the new one offline.
  FakeSocket.all[0].drop(1006);
  expect(phone.connection).toBe("online");
});

test("Try again after a failed try keeps retrying, and a removed token ends in pairing", async () => {
  let status = 0;
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => {
      if (!status) throw new TypeError("Failed to fetch");
      return new Response(JSON.stringify({ error: "This phone is not paired." }), { status });
    }),
  );
  const { connect, phone, reconnectNow } = await client();
  connect();
  FakeSocket.all[0].drop(1006);

  reconnectNow();
  await vi.advanceTimersByTimeAsync(0);
  expect(phone.connection).toBe("offline");

  status = 401;
  await vi.advanceTimersByTimeAsync(3000);
  expect(phone.unpaired).toBe(true);
});
