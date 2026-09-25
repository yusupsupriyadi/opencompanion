import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { ProjectFolder } from "$lib/api";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend } from "../../test/fixtures";
import ChatPage from "./+page.svelte";

const folder = (name: string, path: string): ProjectFolder => ({ name, path, markers: ["git"], source: "root" });
const FOLDERS = [
  folder("uninote", "C:\\Users\\me\\Project\\uninote"),
  folder("ai-remote", "C:\\Users\\me\\Project\\ai-remote"),
  folder("my app", "C:\\Users\\me\\My Projects\\my app"),
];

function chat() {
  return backend({
    chat_threads: () => [],
    project_folders: () => FOLDERS,
    chat_send: (a) => ({
      thread: { id: "t9", title: "x", createdAt: 1, updatedAt: 1 },
      user: { id: "u9", threadId: "t9", role: "user", text: a?.message, cards: [], createdAt: 1 },
      reply: { id: "r9", threadId: "t9", role: "planner", text: "Here is a card.", cards: [], createdAt: 1 },
    }),
  });
}

const box = () => screen.getByLabelText("Message the planner") as HTMLTextAreaElement;
const folders = () => screen.getByRole("listbox", { name: "Folders" });

beforeEach(() => {
  setUrl("/chat");
  app.clis = CLIS;
  app.sessions = [];
  app.settings = null;
});

test("@ lists the project folders, narrows as you type, and Enter writes the path", async () => {
  const api = chat();
  const user = userEvent.setup();
  render(ChatPage);
  await user.type(box(), "Fix the tests in @");
  expect(await within(folders()).findByRole("option", { name: /uninote/ })).toBeInTheDocument();
  expect(box()).toHaveAttribute("aria-controls", "folder-mention-list");

  await user.type(box(), "ai-r");
  const options = within(folders()).getAllByRole("option");
  expect(options.map((o) => o.textContent?.trim())).toEqual(["ai-remote ~/Project/ai-remote", "Browse for a folder…"]);
  expect(options[0]).toHaveAttribute("aria-selected", "true");

  await user.keyboard("{Enter}");
  expect(box().value).toBe("Fix the tests in @C:\\Users\\me\\Project\\ai-remote ");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  expect(api.calls("chat_send")).toEqual([]);

  await user.keyboard("{Enter}");
  expect(api.calls("chat_send")).toEqual([{ threadId: null, message: "Fix the tests in @C:\\Users\\me\\Project\\ai-remote" }]);
});

test("a path with spaces is quoted, and a click picks too", async () => {
  chat();
  const user = userEvent.setup();
  render(ChatPage);
  await user.type(box(), "@my");
  await user.click(await within(folders()).findByRole("option", { name: /my app/ }));
  expect(box().value).toBe('@"C:\\Users\\me\\My Projects\\my app" ');
  expect(box()).toHaveFocus();
});

test("Browse writes the folder picked in the dialog", async () => {
  chat();
  const user = userEvent.setup();
  render(ChatPage);
  await user.type(box(), "look at @zzz");
  expect(await screen.findByText(/No known folder matches "zzz"/)).toBeInTheDocument();
  await user.keyboard("{Enter}");
  await expect.poll(() => box().value).toBe("look at @C:\\Users\\me\\Project\\picked ");
});

test("Escape closes the list and Enter sends again; an email address opens nothing", async () => {
  const api = chat();
  const user = userEvent.setup();
  render(ChatPage);
  await user.type(box(), "mail me@host");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();

  await user.clear(box());
  await user.type(box(), "go @uni");
  await within(folders()).findByRole("option", { name: /uninote/ });
  await user.keyboard("{ArrowDown}");
  expect(within(folders()).getByRole("option", { name: /Browse/ })).toHaveAttribute("aria-selected", "true");
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  await user.keyboard("{Enter}");
  expect(api.calls("chat_send")).toEqual([{ threadId: null, message: "go @uni" }]);
});
