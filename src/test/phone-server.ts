import { vi } from "vitest";

type Handler = (body: Record<string, unknown> | undefined) => [number, unknown];

/** Stands in for the desktop's companion server: `"POST /api/sessions"` → handler. Unknown routes fail loudly. */
export function phoneServer(routes: Record<string, Handler>) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const key = `${init?.method ?? "GET"} ${url}`;
    const handler = routes[key];
    if (!handler) return new Response(JSON.stringify({ error: `No route for ${key}` }), { status: 500 });
    const [status, body] = handler(init?.body ? JSON.parse(init.body as string) : undefined);
    return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

/** The JSON bodies sent to `key`, oldest first. */
export function sent(fetchMock: ReturnType<typeof phoneServer>, key: string) {
  return (fetchMock.mock.calls as unknown as [string, RequestInit | undefined][])
    .filter(([url, init]) => `${init?.method ?? "GET"} ${url}` === key)
    .map(([, init]) => (init?.body ? JSON.parse(init.body as string) : undefined));
}
