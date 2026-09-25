import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { backend, session } from "../test/fixtures";
import NeedsYou from "./NeedsYou.svelte";

const waiting = (canAnswer: boolean) =>
  session({
    status: "waiting",
    waiting: { reason: "permission", tool: "Bash", detail: "npm install", requestId: "r1", canAnswer, method: "stdio", since: Date.now() },
  });

test("Approve and Deny answer the waiting session", async () => {
  const api = backend({ answer_session: () => session() });
  const user = userEvent.setup();
  render(NeedsYou, { s: waiting(true) });
  expect(screen.getByRole("heading", { name: "Claude Code wants to run a command" })).toBeInTheDocument();
  expect(screen.getByText("npm install")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Approve" }));
  await user.click(screen.getByRole("button", { name: "Deny" }));
  expect(api.calls("answer_session")).toEqual([
    { id: "s1", allow: true },
    { id: "s1", allow: false },
  ]);
  expect(screen.getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/session?id=s1");
});

test("prompts OpenCompanion cannot answer point to the terminal instead", () => {
  backend({});
  render(NeedsYou, { s: waiting(false) });
  expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
  expect(screen.getByText(/Answer this in the session's terminal/)).toBeInTheDocument();
});

test("a failed answer is shown next to the buttons", async () => {
  backend({ answer_session: () => new Error("This session has ended.") });
  const user = userEvent.setup();
  render(NeedsYou, { s: waiting(true) });
  await user.click(screen.getByRole("button", { name: "Approve" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("This session has ended.");
});
