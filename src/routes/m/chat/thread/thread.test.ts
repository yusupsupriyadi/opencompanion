import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { ChatMessage, DispatchCard } from "$lib/api";
import { phone, receive } from "$lib/phone.svelte";
import { setUrl } from "../../../../test/app-state.svelte";
import { phoneServer, sent } from "../../../../test/phone-server";
import Thread from "./+page.svelte";

const card: DispatchCard = {
  id: "c1",
  cli: "claude",
  title: "Fix the tests",
  folder: "C:\\Users\\me\\Project\\uninote",
  prompt: "fix the failing tests",
  mode: "headless",
  reason: "uninote has the failing suite",
  problem: null,
  state: "proposed",
  sessionId: null,
};
const thread = { id: "th1", title: "Fix tests in uninote", createdAt: 1, updatedAt: 2 };
const user: ChatMessage = { id: "u1", threadId: "th1", role: "user", text: "fix tests in uninote", cards: [], createdAt: 1 };
const reply: ChatMessage = { id: "r1", threadId: "th1", role: "planner", text: "One session.", cards: [card], createdAt: 2 };

beforeEach(() => {
  phone.sessions = [];
  vi.mocked(goto).mockClear();
  // jsdom has no scrolling; the thread keeps the newest message in view.
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
});

test("the first message starts a chat, and its card runs from the phone", async () => {
  setUrl("/m/chat/thread");
  const fetchMock = phoneServer({
    "POST /api/chat": () => [200, { thread, user, reply }],
    "POST /api/chat/cards/run": () => [200, { message: { ...reply, cards: [{ ...card, state: "started", sessionId: "s7" }] } }],
  });
  const u = userEvent.setup();
  render(Thread);
  expect(screen.getByRole("heading", { name: "New chat" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();

  await u.type(screen.getByLabelText("Message the planner"), "fix tests in uninote");
  await u.click(screen.getByRole("button", { name: "Send" }));
  expect(sent(fetchMock, "POST /api/chat")).toEqual([{ threadId: null, message: "fix tests in uninote" }]);
  expect(await screen.findByRole("heading", { name: "Fix tests in uninote" })).toBeInTheDocument();
  expect(goto).toHaveBeenCalledWith("/m/chat/thread?id=th1", { replaceState: true, keepFocus: true, noScroll: true });

  await u.click(screen.getByRole("button", { name: "Run in uninote" }));
  expect(sent(fetchMock, "POST /api/chat/cards/run")).toEqual([{ messageId: "r1", cardId: "c1" }]);
  expect(await screen.findByRole("link", { name: "Open session" })).toHaveAttribute("href", "/m/session?id=s7");
});

test("a saved chat loads its messages; a discarded card can come back", async () => {
  setUrl("/m/chat/thread?id=th1");
  const fetchMock = phoneServer({
    "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }],
    "POST /api/chat/cards/discard": (b) => [200, { message: { ...reply, cards: [{ ...card, state: b?.undo ? "proposed" : "discarded" }] } }],
  });
  const u = userEvent.setup();
  render(Thread);
  expect(await screen.findByText("fix tests in uninote")).toBeInTheDocument();
  await u.click(screen.getByRole("button", { name: "Discard" }));
  expect(await screen.findByText(/Discarded the Claude Code card for uninote/)).toBeInTheDocument();
  await u.click(screen.getByRole("button", { name: "Undo" }));
  expect(await screen.findByRole("button", { name: "Run in uninote" })).toBeInTheDocument();
  expect(sent(fetchMock, "POST /api/chat/cards/discard")).toEqual([
    { messageId: "r1", cardId: "c1" },
    { messageId: "r1", cardId: "c1", undo: true },
  ]);
});

test("a deleted chat says so", async () => {
  setUrl("/m/chat/thread?id=gone");
  phoneServer({ "GET /api/chat/gone": () => [404, { error: "This chat was deleted." }] });
  render(Thread);
  expect(await screen.findByRole("alert")).toHaveTextContent("This chat was deleted");
  expect(screen.getByRole("link", { name: "Start a new chat" })).toHaveAttribute("href", "/m/chat/thread");
});

test("a chat the planner is still answering says so and holds Send until the answer lands", async () => {
  setUrl("/m/chat/thread?id=th1");
  let busy = true;
  phoneServer({ "GET /api/chat/th1": () => [200, { thread, messages: busy ? [user] : [user, reply], answering: busy }] });
  const typist = userEvent.setup();
  render(Thread);

  expect(await screen.findByText("The planner is still answering the last message. Its answer shows up here.")).toBeInTheDocument();
  await typist.type(screen.getByLabelText("Message the planner"), "and the docs");
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();

  busy = false;
  receive({ type: "chat", threadId: "th1" });
  expect(await screen.findByText("One session.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
});
