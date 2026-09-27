import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { setUrl } from "../../../test/app-state.svelte";
import Pair from "./+page.svelte";

function respond(status: number, body: unknown) {
  return vi.fn(async () => new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } }));
}

beforeEach(() => {
  localStorage.clear();
  vi.mocked(goto).mockClear();
});

test("a code from the QR is filled in and pairing stores the token", async () => {
  setUrl("/m/pair?code=482913");
  const fetchMock = vi.fn(async (url: string) => {
    if (url === "/api/pair") return new Response(JSON.stringify({ token: "tok-1" }), { status: 200 });
    return new Response(JSON.stringify({ sessions: [] }), { status: 200 });
  });
  vi.stubGlobal("fetch", fetchMock);
  globalThis.WebSocket = class {
    close() {}
  } as unknown as typeof WebSocket;
  render(Pair);
  expect(screen.getByLabelText("Digit 1")).toHaveValue("4");
  expect(screen.getByLabelText("Digit 6")).toHaveValue("3");
  await userEvent.setup().click(screen.getByRole("button", { name: "Pair" }));
  const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
  expect(url).toBe("/api/pair");
  expect(JSON.parse(init.body as string)).toMatchObject({ code: "482913" });
  expect(localStorage.getItem("air-token")).toBe("tok-1");
  expect(goto).toHaveBeenCalledWith("/m");
});

test("a wrong code shows the desktop's reason", async () => {
  setUrl("/m/pair");
  vi.stubGlobal("fetch", respond(401, { error: "That code is not right." }));
  const user = userEvent.setup();
  render(Pair);
  for (let i = 1; i <= 6; i++) await user.type(screen.getByLabelText(`Digit ${i}`), String(i));
  await user.click(screen.getByRole("button", { name: "Pair" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("That code is not right.");
  expect(localStorage.getItem("air-token")).toBeNull();
});

test("the Home Screen app on an iPhone asks for the typed code and names itself apart from Safari", () => {
  setUrl("/m/pair");
  vi.stubGlobal("fetch", respond(200, {}));
  const ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148";
  Object.defineProperty(navigator, "userAgent", { value: ua, configurable: true });
  Object.defineProperty(navigator, "standalone", { value: true, configurable: true });
  try {
    render(Pair);
    expect(screen.getByText(/keeps its own pairing, apart from Safari/)).toBeInTheDocument();
    expect(screen.queryByText(/scan the code there/)).not.toBeInTheDocument();
    expect(screen.getByLabelText("Name for this phone")).toHaveValue("iPhone · Home Screen");
  } finally {
    delete (navigator as { userAgent?: string }).userAgent;
    delete (navigator as { standalone?: boolean }).standalone;
  }
});

test("an incomplete code is caught before anything is sent", async () => {
  setUrl("/m/pair");
  const fetchMock = respond(200, {});
  vi.stubGlobal("fetch", fetchMock);
  const user = userEvent.setup();
  render(Pair);
  await user.type(screen.getByLabelText("Digit 1"), "1");
  await user.click(screen.getByRole("button", { name: "Pair" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Enter all 6 digits");
  expect(fetchMock).not.toHaveBeenCalled();
});
