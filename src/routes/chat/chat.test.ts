import { render, screen, waitFor, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { ChatMessage, ChatThread, Settings } from "$lib/api";
import { app, forgetModelLists } from "$lib/store.svelte";
import { page, setUrl } from "../../test/app-state.svelte";
import { CLIS, backend } from "../../test/fixtures";
import ChatPage from "./+page.svelte";

const now = Date.now();
const docs: ChatThread = { id: "t2", title: "Write the API docs in uninote", createdAt: 1, updatedAt: now - 60_000 };
const tests: ChatThread = { id: "t1", title: "Codex: fix the failing tests", createdAt: 1, updatedAt: now - 3 * 3_600_000 };

const said = (threadId: string, id: string, role: ChatMessage["role"], text: string): ChatMessage => ({
  id,
  threadId,
  role,
  text,
  cards: [],
  createdAt: 1,
});

const history: Record<string, ChatMessage[]> = {
  t1: [said("t1", "a1", "user", "Codex: fix the failing tests"), said("t1", "a2", "planner", "One card for ai-remote.")],
  t2: [said("t2", "b1", "user", "Write the API docs in uninote"), said("t2", "b2", "planner", "Claude Code in uninote.")],
};

function chats(threads: ChatThread[] = [docs, tests]) {
  return backend({
    chat_threads: () => threads,
    chat_history: (a) => history[a?.threadId as string] ?? [],
    chat_send: (a) => {
      const thread = a?.threadId ? threads.find((t) => t.id === a.threadId)! : { id: "t9", title: a?.message as string, createdAt: 5, updatedAt: 5 };
      return { thread, user: said(thread.id, "u9", "user", a?.message as string), reply: said(thread.id, "r9", "planner", "Here is a card.") };
    },
    chat_delete_thread: () => undefined,
  });
}

const list = () => within(screen.getByRole("navigation", { name: "Chats" }));

beforeEach(() => {
  app.clis = CLIS;
  app.sessions = [];
  app.settings = null;
  app.now = now;
});

test("the list shows every chat, newest first, and opens the one in the address", async () => {
  setUrl("/chat?id=t1");
  const api = chats();
  render(ChatPage);
  expect(await screen.findByText("One card for ai-remote.")).toBeInTheDocument();
  const links = list().getAllByRole("link");
  expect(links.map((l) => l.getAttribute("href"))).toEqual(["/chat?id=t2", "/chat?id=t1"]);
  expect(list().getByRole("link", { name: /fix the failing tests/ })).toHaveAttribute("aria-current", "page");
  expect(list().getByRole("link", { name: /API docs/ })).toHaveTextContent("1 min ago");
  expect(api.calls("chat_history")).toEqual([{ threadId: "t1" }]);
  expect(screen.queryByText("Claude Code in uninote.")).not.toBeInTheDocument();
});

test("switching chats shows only that chat's messages", async () => {
  setUrl("/chat?id=t1");
  chats();
  render(ChatPage);
  await screen.findByText("One card for ai-remote.");
  setUrl("/chat?id=t2");
  expect(await screen.findByText("Claude Code in uninote.")).toBeInTheDocument();
  expect(screen.queryByText("One card for ai-remote.")).not.toBeInTheDocument();

  await userEvent.setup().click(screen.getByRole("button", { name: "New chat" }));
  expect(page.url.search).toBe("");
  await waitFor(() => expect(screen.queryByText("Claude Code in uninote.")).not.toBeInTheDocument());
  expect(screen.queryByText(/Describe a task and where it should happen/)).not.toBeInTheDocument();
  expect(screen.getByLabelText("Message the planner")).toHaveFocus();
});

test("the first message of a new chat starts a chat and opens it", async () => {
  setUrl("/chat");
  const api = chats([]);
  const user = userEvent.setup();
  render(ChatPage);
  expect(await list().findByText(/No chats yet/)).toBeInTheDocument();
  await user.type(screen.getByLabelText("Message the planner"), "Add a dark mode toggle to uninote{Enter}");
  expect(await screen.findByText("Here is a card.")).toBeInTheDocument();
  expect(api.calls("chat_send")).toEqual([{ threadId: null, message: "Add a dark mode toggle to uninote" }]);
  expect(list().getByRole("link", { name: /dark mode toggle/ })).toHaveAttribute("aria-current", "page");
  expect(page.url.search).toBe("?id=t9");
  expect(api.calls("chat_history")).toEqual([]);
});

test("a message in an open chat goes to that chat", async () => {
  setUrl("/chat?id=t2");
  const api = chats();
  const user = userEvent.setup();
  render(ChatPage);
  await screen.findByText("Claude Code in uninote.");
  await user.type(screen.getByLabelText("Message the planner"), "Also the README{Enter}");
  expect(await screen.findByText("Here is a card.")).toBeInTheDocument();
  expect(api.calls("chat_send")).toEqual([{ threadId: "t2", message: "Also the README" }]);
});

test("deleting a chat takes a second press and leaves the others", async () => {
  setUrl("/chat?id=t1");
  const api = chats();
  const user = userEvent.setup();
  render(ChatPage);
  await screen.findByText("One card for ai-remote.");
  await user.click(screen.getByRole("button", { name: "Delete chat" }));
  expect(api.calls("chat_delete_thread")).toEqual([]);
  await user.click(screen.getByRole("button", { name: "Press again to delete" }));
  expect(api.calls("chat_delete_thread")).toEqual([{ threadId: "t1" }]);
  expect(page.url.search).toBe("");
  expect(list().getAllByRole("link").map((l) => l.getAttribute("href"))).toEqual(["/chat?id=t2"]);
  expect(screen.queryByRole("button", { name: "Delete chat" })).not.toBeInTheDocument();
});

test("a chat that no longer exists says so instead of an empty thread", async () => {
  setUrl("/chat?id=gone");
  chats();
  render(ChatPage);
  expect(await screen.findByRole("heading", { name: "This chat is gone" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
});

test("a list that fails to load offers another try", async () => {
  setUrl("/chat");
  backend({ chat_threads: () => new Error("database is locked") });
  render(ChatPage);
  expect(await screen.findByText(/database is locked/)).toBeInTheDocument();
  expect(list().getByRole("button", { name: "Try again" })).toBeInTheDocument();
});

test("the composer offers the planner's models and thinking levels, but not for a custom provider", async () => {
  setUrl("/chat");
  forgetModelLists();
  backend({
    chat_threads: () => [],
    chat_models: () => ({ models: [{ id: "opus", label: "Opus", group: null, efforts: ["low", "high"] }], defaultEfforts: ["low", "high"] }),
  });
  app.settings = { chatCli: "claude", plannerSource: "cli", chatModels: { claude: { model: "opus", effort: "high" } } } as unknown as Settings;
  const { unmount } = render(ChatPage);
  expect(await screen.findByRole("option", { name: "Opus" })).toBeInTheDocument();
  expect(screen.getByLabelText("Model")).toHaveValue("opus");
  expect(screen.getByLabelText("Thinking")).toHaveValue("high");
  unmount();

  app.settings = { ...app.settings, plannerSource: "api" } as Settings;
  render(ChatPage);
  expect(await list().findByText(/No chats yet/)).toBeInTheDocument();
  expect(screen.queryByLabelText("Model")).not.toBeInTheDocument();
});

test("a custom provider plans with no CLI installed, and one without a model cannot send", async () => {
  setUrl("/chat");
  app.clis = CLIS.map((c) => ({ ...c, path: null }));
  app.settings = {
    chatCli: null,
    plannerSource: "api",
    plannerApi: { baseUrl: "http://localhost:11434/v1", model: "qwen3-coder", apiKey: "" },
  } as unknown as Settings;
  const api = chats([]);
  const user = userEvent.setup();
  const { unmount } = render(ChatPage);
  expect(await list().findByText(/No chats yet/)).toBeInTheDocument();
  expect(screen.queryByText(/Planner: qwen3-coder/)).not.toBeInTheDocument();
  expect(screen.queryByText(/custom provider has no base URL/)).not.toBeInTheDocument();
  await user.type(screen.getByLabelText("Message the planner"), "Add a dark mode toggle to uninote{Enter}");
  expect(await screen.findByText("Here is a card.")).toBeInTheDocument();
  expect(api.calls("chat_send")).toHaveLength(1);
  unmount();

  app.settings = { ...app.settings, plannerApi: { baseUrl: "http://localhost:11434/v1", model: "", apiKey: "" } } as Settings;
  setUrl("/chat");
  render(ChatPage);
  expect(await screen.findByText("The custom provider has no base URL or model yet. Add them in Settings.")).toBeInTheDocument();
  await user.type(screen.getByLabelText("Message the planner"), "Fix the tests");
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
});

test("with no CLI that can plan, the composer says why Send is off", async () => {
  setUrl("/chat");
  app.clis = CLIS.map((c) => ({ ...c, path: null }));
  app.clisState = "ready";
  chats([]);
  const user = userEvent.setup();
  render(ChatPage);
  expect(await screen.findByText("No CLI that can plan is installed.")).toBeInTheDocument();
  await user.type(screen.getByLabelText("Message the planner"), "Fix the tests");
  expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
});
