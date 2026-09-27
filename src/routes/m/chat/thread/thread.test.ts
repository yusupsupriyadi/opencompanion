import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { ChatMessage, DispatchCard } from "$lib/api";
import { i18n } from "$lib/i18n.svelte";
import { phone, receive } from "$lib/phone.svelte";
import { setUrl } from "../../../../test/app-state.svelte";
import { CLIS, session } from "../../../../test/fixtures";
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
const other = { id: "th2", title: "Docs for the API", createdAt: 1, updatedAt: 3 };

// Claude Code plans; Codex CLI is installed too, OpenCode and Gemini CLI are not.
const SETUP = { clis: CLIS, chatCli: "claude", plannerSource: "cli", provider: null, chatModels: {}, autoRun: [] as string[] };
const CLAUDE_MODELS = { models: [{ id: "opus", label: "Opus", group: null, efforts: ["low", "high"] }], defaultEfforts: ["low", "high"] };

type Routes = Parameters<typeof phoneServer>[0];

/** The companion server with a ready planner; `routes` adds to it or replaces a route. */
function server(routes: Routes = {}) {
  return phoneServer({
    "GET /api/chat/threads": () => [200, { threads: [thread], answering: [] }],
    "GET /api/chat/setup": () => [200, SETUP],
    "GET /api/chat/models/claude": () => [200, CLAUDE_MODELS],
    ...routes,
  });
}

// Links in a sheet run their own click handler; jsdom cannot follow them, so the test moves the address itself.
const stayPut = (e: Event) => {
  if (e.target instanceof Element && e.target.closest("a[href]")) e.preventDefault();
};

beforeEach(() => {
  phone.sessions = [];
  phone.notice = "";
  vi.mocked(goto).mockClear();
  // jsdom has no scrolling; the thread keeps the newest message in view.
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  document.addEventListener("click", stayPut);
});

afterEach(() => {
  document.removeEventListener("click", stayPut);
  vi.restoreAllMocks();
  i18n.lang = "en";
});

test("the first message starts a chat, and its card runs from the phone", async () => {
  setUrl("/m/chat/thread");
  const fetchMock = server({
    "POST /api/chat": () => [200, { thread, user, reply }],
    "POST /api/chat/cards/run": () => [200, { message: { ...reply, cards: [{ ...card, state: "started", sessionId: "s7" }] } }],
  });
  const u = userEvent.setup();
  render(Thread);
  expect(screen.getByRole("heading", { name: "New chat" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();

  await u.type(screen.getByLabelText("Message the planner"), "fix tests in uninote");
  await u.click(await screen.findByRole("button", { name: "Send" }));
  expect(sent(fetchMock, "POST /api/chat")).toEqual([{ threadId: null, message: "fix tests in uninote" }]);
  expect(await screen.findByRole("heading", { name: "Fix tests in uninote" })).toBeInTheDocument();
  expect(goto).toHaveBeenCalledWith("/m/chat/thread?id=th1", { replaceState: true, keepFocus: true, noScroll: true });

  await u.click(screen.getByRole("button", { name: "Run in uninote" }));
  expect(sent(fetchMock, "POST /api/chat/cards/run")).toEqual([{ messageId: "r1", cardId: "c1" }]);
  expect(await screen.findByRole("link", { name: "Open session" })).toHaveAttribute("href", "/m/session?id=s7");
});

test("a saved chat loads its messages; a discarded card can come back", async () => {
  setUrl("/m/chat/thread?id=th1");
  const fetchMock = server({
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
  server({ "GET /api/chat/gone": () => [404, { error: "This chat was deleted." }] });
  render(Thread);
  expect(await screen.findByRole("alert")).toHaveTextContent("This chat was deleted");
  expect(screen.getByRole("link", { name: "Start a new chat" })).toHaveAttribute("href", "/m/chat/thread");
});

test("a chat the planner is still answering says so and holds Send and Delete until the answer lands", async () => {
  setUrl("/m/chat/thread?id=th1");
  let busy = true;
  server({
    "GET /api/chat/th1": () => [200, { thread, messages: busy ? [user] : [user, reply], answering: busy }],
    "GET /api/chat/threads": () => [200, { threads: [thread], answering: busy ? ["th1"] : [] }],
  });
  const typist = userEvent.setup();
  render(Thread);

  expect(await screen.findByText("The planner is still answering the last message. Its answer shows up here.")).toBeInTheDocument();
  await typist.type(screen.getByLabelText("Message the planner"), "and the docs");
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Delete chat" })).toBeDisabled();

  busy = false;
  receive({ type: "chat", threadId: "th1" });
  expect(await screen.findByText("One session.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Delete chat" })).toBeEnabled();
  expect(document.getElementById("chat-announcement")).toHaveTextContent("The planner answered with 1 session card.");
});

test("the chat history opens as a sheet, and another chat opens in place", async () => {
  setUrl("/m/chat/thread?id=th1");
  server({
    "GET /api/chat/threads": () => [200, { threads: [other, thread], answering: ["th2"] }],
    "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }],
    "GET /api/chat/th2": () => [200, { thread: other, messages: [], answering: true }],
  });
  const u = userEvent.setup();
  render(Thread);
  await screen.findByText("fix tests in uninote");

  await u.click(screen.getByRole("button", { name: "Chat history" }));
  const sheet = screen.getByRole("dialog", { name: "Chat history" });
  expect(within(sheet).getByRole("link", { name: /Fix tests in uninote/ })).toHaveAttribute("aria-current", "page");
  const docs = within(sheet).getByRole("link", { name: /Docs for the API/ });
  expect(docs).toHaveAttribute("href", "/m/chat/thread?id=th2");
  expect(within(docs).getByText("Planner is answering…")).toBeInTheDocument();
  expect(within(sheet).getByRole("link", { name: "New chat" })).toHaveAttribute("href", "/m/chat/thread");

  await u.click(docs);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  setUrl("/m/chat/thread?id=th2");
  expect(await screen.findByRole("heading", { level: 1, name: "Docs for the API" })).toBeInTheDocument();
  expect(await screen.findByText("The planner is still answering the last message. Its answer shows up here.")).toBeInTheDocument();
});

test("Delete chat asks twice, then leaves for the chat list", async () => {
  setUrl("/m/chat/thread?id=th1");
  const fetchMock = server({
    "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }],
    "DELETE /api/chat/th1": () => [200, { ok: true }],
  });
  const u = userEvent.setup();
  render(Thread);
  await u.click(await screen.findByRole("button", { name: "Delete chat" }));
  expect(sent(fetchMock, "DELETE /api/chat/th1")).toEqual([]);
  await u.click(screen.getByRole("button", { name: "Press again to delete" }));
  expect(sent(fetchMock, "DELETE /api/chat/th1")).toHaveLength(1);
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/m/chat", { replaceState: true }));
  expect(phone.notice).toBe('Deleted "Fix tests in uninote".');
});

test("a chat deleted on the computer while it is open says so", async () => {
  setUrl("/m/chat/thread?id=th1");
  let gone = false;
  server({ "GET /api/chat/th1": () => (gone ? [404, { error: "This chat was deleted." }] : [200, { thread, messages: [user, reply] }]) });
  render(Thread);
  await screen.findByText("One session.");
  gone = true;
  receive({ type: "chat", threadId: "th1" });
  expect(await screen.findByRole("alert")).toHaveTextContent("This chat was deleted");
  expect(screen.queryByRole("button", { name: "Delete chat" })).not.toBeInTheDocument();
});

test("the live sessions open as a sheet, each with its horizon, place and last step", async () => {
  setUrl("/m/chat/thread");
  phone.sessions = [
    session({ id: "s1", status: "running", title: "Add a dark mode toggle", lastEvent: "Write a.txt", startedAt: Date.now() - 60_000 }),
    session({ id: "s2", status: "done", title: "Old work" }),
  ];
  const fetchMock = server({
    "GET /api/sessions": () => [200, { sessions: phone.sessions, marks: { s1: [[Date.now() - 30_000, "tool_call"]] } }],
  });
  const u = userEvent.setup();
  render(Thread);

  await u.click(screen.getByRole("button", { name: "Live sessions: 1 running" }));
  const sheet = screen.getByRole("dialog", { name: "Live sessions" });
  const link = within(sheet).getByRole("link", { name: /Add a dark mode toggle/ });
  expect(link).toHaveAttribute("href", "/m/session?id=s1");
  expect(within(link).getByText("Running")).toBeInTheDocument();
  expect(within(link).getByText("Claude Code in uninote · Write a.txt")).toBeInTheDocument();
  expect(within(sheet).queryByText("Old work")).not.toBeInTheDocument();
  expect(sent(fetchMock, "GET /api/sessions")).toHaveLength(1);
  expect(await within(link).findByRole("img", { name: "Activity: 1 commands, file edits and approvals" })).toBeInTheDocument();

  // A new step lands on the horizon while the sheet is open.
  receive({ type: "event", event: { id: 9, sessionId: "s1", at: Date.now(), event: { kind: "file_changed", path: "a.txt" } } });
  expect(await within(link).findByRole("img", { name: "Activity: 2 commands, file edits and approvals" })).toBeInTheDocument();
});

test("@ lists the computer's project folders and writes the chosen path; Browse stays on the desktop", async () => {
  setUrl("/m/chat/thread");
  server({
    "GET /api/folders": () => [
      200,
      {
        folders: [
          { path: "C:\\Users\\me\\Project\\uninote", name: "uninote", markers: [], source: "recent" },
          { path: "C:\\Users\\me\\Project\\ai-remote", name: "ai-remote", markers: [], source: "recent" },
        ],
      },
    ],
  });
  const u = userEvent.setup();
  render(Thread);
  const box = screen.getByLabelText("Message the planner");
  await u.type(box, "fix @uni");
  const list = await screen.findByRole("listbox", { name: "Folders" });
  expect(within(list).getAllByRole("option")).toHaveLength(1);
  expect(screen.queryByText("Browse for a folder…")).not.toBeInTheDocument();
  await u.click(within(list).getByRole("option", { name: /uninote/ }));
  expect(box).toHaveValue("fix @C:\\Users\\me\\Project\\uninote ");

  await u.type(box, "@zzz");
  expect(await screen.findByText('No known folder matches "zzz". Keep typing its full path.')).toBeInTheDocument();
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});

test("the planner's model picker keeps the choice on the computer", async () => {
  setUrl("/m/chat/thread");
  const fetchMock = server({
    "POST /api/chat/model": (b) => [200, { chatModels: { claude: { model: b?.model, effort: b?.effort } } }],
  });
  const u = userEvent.setup();
  render(Thread);
  await screen.findByRole("option", { name: "Opus" });
  await u.selectOptions(screen.getByLabelText("Model"), "opus");
  expect(sent(fetchMock, "POST /api/chat/model")).toEqual([{ cli: "claude", model: "opus", effort: "" }]);
  expect((screen.getByLabelText("Model") as HTMLSelectElement).value).toBe("opus");
});

test("a custom provider plans without the model picker, and says when it is not set up", async () => {
  setUrl("/m/chat/thread");
  let ready = true;
  server({ "GET /api/chat/setup": () => [200, { ...SETUP, plannerSource: "api", provider: { model: "qwen3", ready } }] });
  const u = userEvent.setup();
  const view = render(Thread);
  await u.type(screen.getByLabelText("Message the planner"), "fix tests");
  expect(await screen.findByRole("button", { name: "Send" })).toBeEnabled();
  expect(screen.queryByLabelText("Model")).not.toBeInTheDocument();
  view.unmount();

  ready = false;
  render(Thread);
  expect(await screen.findByText("The custom provider has no base URL or model yet. Add them in Settings.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
});

test("the composer says why nothing can plan, and holds Send", async () => {
  setUrl("/m/chat/thread");
  server({ "GET /api/chat/setup": () => [200, { ...SETUP, clis: CLIS.map((c) => ({ ...c, path: null })) }] });
  const u = userEvent.setup();
  render(Thread);
  expect(screen.getByText("Checking which CLI can plan…")).toBeInTheDocument();
  await u.type(screen.getByLabelText("Message the planner"), "fix tests");
  expect(await screen.findByText("No CLI that can plan is installed.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
});

test("while the planner reads one chat, it names the CLI there and another chat says why Send waits", async () => {
  setUrl("/m/chat/thread?id=th1");
  const fetchMock = server({
    "GET /api/chat/threads": () => [200, { threads: [other, thread], answering: [] }],
    "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }],
    "GET /api/chat/th2": () => [200, { thread: other, messages: [] }],
  });
  let answer = () => {};
  const answered = new Promise<void>((resolve) => (answer = resolve));
  const route = fetchMock.getMockImplementation()!;
  fetchMock.mockImplementation(async (url: string, init?: RequestInit) => {
    if (init?.method !== "POST" || url !== "/api/chat") return route(url, init);
    await answered;
    return new Response(JSON.stringify({ thread, user: { ...user, id: "u2", text: "and the docs" }, reply: { ...reply, id: "r2" } }), { status: 200 });
  });
  const u = userEvent.setup();
  render(Thread);
  await screen.findByText("One session.");
  await screen.findByLabelText("Model");
  await u.type(screen.getByLabelText("Message the planner"), "and the docs");
  await u.click(screen.getByRole("button", { name: "Send" }));
  expect(await screen.findByText("Claude Code is reading your message. This usually takes 10 to 30 seconds.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Delete chat" })).toBeDisabled();

  setUrl("/m/chat/thread?id=th2");
  expect(await screen.findByText("The planner is still answering in another chat. Send works again when it is done.")).toBeInTheDocument();
  await u.type(screen.getByLabelText("Message the planner"), "docs please");
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();

  answer();
  await vi.waitFor(() => expect(screen.getByRole("button", { name: "Send" })).toBeEnabled());
  expect(screen.queryByText(/still answering in another chat/)).not.toBeInTheDocument();
});

test("Enter adds a line on a touch keyboard and sends from a hardware keyboard", async () => {
  setUrl("/m/chat/thread");
  const fetchMock = server({ "POST /api/chat": () => [200, { thread, user, reply }] });
  const u = userEvent.setup();
  render(Thread);
  await screen.findByLabelText("Model");
  const box = screen.getByLabelText("Message the planner");
  await u.type(box, "fix tests{Enter}in uninote");
  expect(box).toHaveValue("fix tests\nin uninote");
  expect(sent(fetchMock, "POST /api/chat")).toEqual([]);

  // A tablet with a keyboard case reports a fine pointer.
  const media = window.matchMedia;
  vi.spyOn(window, "matchMedia").mockImplementation((q: string) => ({ ...media(q), matches: q === "(pointer: fine)" }));
  await u.type(box, "{Enter}");
  expect(sent(fetchMock, "POST /api/chat")).toEqual([{ threadId: null, message: "fix tests\nin uninote" }]);
});

test("a card can be edited before Run: CLI, mode and prompt, checked on the computer", async () => {
  setUrl("/m/chat/thread?id=th1");
  const fetchMock = server({
    "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }],
    "POST /api/chat/cards/edit": (b) => [200, { message: { ...reply, cards: [b?.card] } }],
  });
  const u = userEvent.setup();
  render(Thread);
  await screen.findByLabelText("Model");
  await u.click(await screen.findByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Title")).toHaveFocus();
  expect(within(screen.getByLabelText("CLI")).getByRole("option", { name: "OpenCode (not installed)" })).toBeDisabled();
  await u.selectOptions(screen.getByLabelText("CLI"), "codex");
  await u.selectOptions(screen.getByLabelText("Mode"), "interactive");
  await u.clear(screen.getByLabelText("Prompt"));
  await u.type(screen.getByLabelText("Prompt"), "fix the login tests");
  await u.click(screen.getByRole("button", { name: "Save card" }));

  expect(sent(fetchMock, "POST /api/chat/cards/edit")).toEqual([
    { messageId: "r1", card: { ...card, cli: "codex", mode: "interactive", prompt: "fix the login tests" } },
  ]);
  expect(await screen.findByText("fix the login tests")).toBeInTheDocument();
  expect(screen.getByText(/Codex CLI · interactive/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Run in uninote" })).toHaveFocus();
});

test("a follow-up edits only its message and has no Board card", async () => {
  setUrl("/m/chat/thread?id=th1");
  const follow: DispatchCard = { ...card, id: "c2", target: "s1", title: "Add a dark mode toggle", prompt: "also add tests" };
  phone.sessions = [session({ id: "s1", status: "idle" })];
  server({ "GET /api/chat/th1": () => [200, { thread, messages: [user, { ...reply, cards: [follow] }] }] });
  const u = userEvent.setup();
  render(Thread);
  expect(await screen.findByRole("article", { name: "Follow-up for Add a dark mode toggle" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send to session" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Add to board" })).not.toBeInTheDocument();
  await u.click(screen.getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Message for the session")).toHaveValue("also add tests");
  expect(screen.queryByLabelText("CLI")).not.toBeInTheDocument();
  await u.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.getByRole("button", { name: "Send to session" })).toHaveFocus();
});

test("a Settings change on the computer reaches the open chat", async () => {
  setUrl("/m/chat/thread");
  let autoRun: string[] = [];
  server({ "GET /api/chat/setup": () => [200, { ...SETUP, autoRun }] });
  render(Thread);
  expect(await screen.findByText("Nothing starts until you press Run on a card.")).toBeInTheDocument();
  autoRun = ["C:\\Users\\me\\Project\\uninote"];
  receive({ type: "settings" });
  expect(await screen.findByText("Cards for uninote start without asking; the rest wait for Run.")).toBeInTheDocument();
});

test("the chat speaks Indonesian when the desktop does", async () => {
  i18n.lang = "id";
  setUrl("/m/chat/thread?id=th1");
  phone.sessions = [session({ id: "s1", status: "running" })];
  server({ "GET /api/chat/th1": () => [200, { thread, messages: [user, reply] }] });
  render(Thread);
  expect(await screen.findByRole("button", { name: "Hapus chat" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Riwayat chat" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Sesi yang berjalan: 1" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Ubah" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Kirim" })).toBeInTheDocument();
});
