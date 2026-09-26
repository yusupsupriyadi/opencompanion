import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { CLIS, backend } from "../test/fixtures";
import type { ChatMessage, DispatchCard as Card } from "./api";
import DispatchCard from "./DispatchCard.svelte";
import { app } from "./store.svelte";

const card = (over: Partial<Card> = {}): Card => ({
  id: "c1",
  cli: "claude",
  title: "README run section",
  folder: "C:\\p\\comic-translate",
  prompt: "Add a Run it locally section to README.md",
  mode: "headless",
  reason: "Docs only, so headless is enough.",
  problem: null,
  state: "proposed",
  sessionId: null,
  ...over,
});
const msg = (c: Card): ChatMessage => ({ id: "m1", threadId: "t1", role: "planner", text: "", cards: [c], createdAt: 1 });

beforeEach(() => {
  app.clis = CLIS;
  app.sessions = [];
});

test("Run starts the card's session and reports the updated message", async () => {
  const started = card({ state: "started", sessionId: "s9" });
  const api = backend({ chat_run_card: () => msg(started) });
  const onchange = vi.fn();
  render(DispatchCard, { card: card(), messageId: "m1", onchange });
  await userEvent.setup().click(screen.getByRole("button", { name: "Run in comic-translate" }));
  expect(api.calls("chat_run_card")).toEqual([{ messageId: "m1", cardId: "c1" }]);
  expect(onchange).toHaveBeenCalledWith(msg(started));
});

test("a card with a problem shows it and cannot run", () => {
  backend({});
  render(DispatchCard, { card: card({ problem: "The folder C:\\p\\x does not exist." }), messageId: "m1", onchange: vi.fn() });
  expect(screen.getByText("The folder C:\\p\\x does not exist.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Run in comic-translate" })).toBeDisabled();
});

test("the card is named after its task, or after its CLI when it has no name", () => {
  backend({});
  const { unmount } = render(DispatchCard, { card: card(), messageId: "m1", onchange: vi.fn() });
  expect(screen.getByRole("article", { name: "README run section" })).toHaveTextContent("Claude Code · headless");
  unmount();
  render(DispatchCard, { card: card({ title: "" }), messageId: "m1", onchange: vi.fn() });
  expect(screen.getByRole("article", { name: "Claude Code · headless" })).toBeInTheDocument();
});

test("Edit saves the new title and prompt through validation", async () => {
  const api = backend({ chat_update_card: (a) => msg(a?.card as Card) });
  const user = userEvent.setup();
  render(DispatchCard, { card: card(), messageId: "m1", onchange: vi.fn() });
  await user.click(screen.getByRole("button", { name: "Edit" }));
  const title = screen.getByLabelText("Title");
  await user.clear(title);
  await user.type(title, "README only");
  const prompt = screen.getByLabelText("Prompt");
  await user.clear(prompt);
  await user.type(prompt, "Only touch README.md");
  await user.click(screen.getByRole("button", { name: "Save card" }));
  expect(api.calls("chat_update_card")[0]).toMatchObject({ messageId: "m1", card: { id: "c1", title: "README only", prompt: "Only touch README.md" } });
});

test("Discard and Add to board call the backend", async () => {
  const onBoard = msg(card({ taskId: "t7" }));
  const api = backend({ chat_discard_card: () => msg(card({ state: "discarded" })), chat_card_to_board: () => onBoard });
  const onchange = vi.fn();
  const user = userEvent.setup();
  render(DispatchCard, { card: card(), messageId: "m1", onchange });
  await user.click(screen.getByRole("button", { name: "Add to board" }));
  expect(onchange).toHaveBeenCalledWith(onBoard);
  await user.click(screen.getByRole("button", { name: "Discard" }));
  expect(api.calls("chat_card_to_board")).toEqual([{ messageId: "m1", cardId: "c1" }]);
  expect(api.calls("chat_discard_card")).toEqual([{ messageId: "m1", cardId: "c1", undo: false }]);
});

test("a card already on the Board links there instead of adding a second one", () => {
  backend({});
  render(DispatchCard, { card: card({ taskId: "t7" }), messageId: "m1", onchange: vi.fn() });
  expect(screen.getByRole("link", { name: "On the Board" })).toHaveAttribute("href", "/board");
  expect(screen.queryByRole("button", { name: "Add to board" })).toBeNull();
});

test("a discarded card offers Undo", async () => {
  const api = backend({ chat_discard_card: () => msg(card()) });
  render(DispatchCard, { card: card({ state: "discarded" }), messageId: "m1", onchange: vi.fn() });
  await userEvent.setup().click(screen.getByRole("button", { name: "Undo" }));
  expect(api.calls("chat_discard_card")).toEqual([{ messageId: "m1", cardId: "c1", undo: true }]);
});

test("a started card links to its session", () => {
  backend({});
  render(DispatchCard, { card: card({ state: "started", sessionId: "s9" }), messageId: "m1", onchange: vi.fn() });
  expect(screen.getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/session?id=s9");
  expect(screen.queryByRole("button", { name: /Run in/ })).toBeNull();
});
