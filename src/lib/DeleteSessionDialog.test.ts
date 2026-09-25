import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import { page, setUrl } from "../test/app-state.svelte";
import { backend, session } from "../test/fixtures";
import DeleteSessionDialog from "./DeleteSessionDialog.svelte";
import SessionRow from "./SessionRow.svelte";
import { app, pendingDelete, toast } from "./store.svelte";

const done = session({ id: "d", title: "Docs", status: "done", endedAt: Date.now() });

beforeEach(() => {
  app.sessions = [done];
  pendingDelete.session = null;
  setUrl("/");
});

test("only finished sessions offer Delete", () => {
  backend({});
  render(SessionRow, { s: session({ id: "r", title: "Fix tests", status: "running" }) });
  expect(screen.queryByRole("button", { name: /Delete session/ })).not.toBeInTheDocument();
  render(SessionRow, { s: session({ id: "i", title: "Idle one", status: "idle" }) });
  expect(screen.queryByRole("button", { name: /Delete session/ })).not.toBeInTheDocument();
  render(SessionRow, { s: done });
  expect(screen.getByRole("button", { name: "Delete session: Docs" })).toBeInTheDocument();
});

test("Keep session closes the dialog without deleting", async () => {
  const api = backend({ delete_session: () => undefined });
  const user = userEvent.setup();
  render(SessionRow, { s: done });
  render(DeleteSessionDialog);
  await user.click(screen.getByRole("button", { name: "Delete session: Docs" }));
  expect(screen.getByRole("heading", { name: "Delete this session?" })).toBeInTheDocument();
  expect(screen.getByText(/Files that Claude Code changed in uninote stay as they are/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Keep session" }));
  expect(screen.queryByRole("heading", { name: "Delete this session?" })).not.toBeInTheDocument();
  expect(api.calls("delete_session")).toEqual([]);
  expect(app.sessions).toHaveLength(1);
});

test("confirming deletes the session and drops it from every list", async () => {
  const api = backend({ delete_session: () => undefined });
  const user = userEvent.setup();
  render(SessionRow, { s: done });
  render(DeleteSessionDialog);
  await user.click(screen.getByRole("button", { name: "Delete session: Docs" }));
  await user.click(screen.getByRole("button", { name: "Delete session" }));
  expect(api.calls("delete_session")).toEqual([{ id: "d" }]);
  expect(app.sessions).toEqual([]);
  expect(toast.text).toBe('Deleted "Docs".');
  expect(screen.queryByRole("heading", { name: "Delete this session?" })).not.toBeInTheDocument();
});

test("a refused delete keeps the dialog open and says why", async () => {
  backend({ delete_session: () => new Error("Stop this session before deleting it.") });
  const user = userEvent.setup();
  render(DeleteSessionDialog);
  pendingDelete.session = done;
  await user.click(await screen.findByRole("button", { name: "Delete session" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Stop this session before deleting it.");
  expect(screen.getByRole("heading", { name: "Delete this session?" })).toBeInTheDocument();
  expect(app.sessions).toHaveLength(1);
});

test("deleting the session that is open goes back to Overview", async () => {
  backend({ delete_session: () => undefined });
  setUrl("/session?id=d");
  const user = userEvent.setup();
  render(DeleteSessionDialog);
  pendingDelete.session = done;
  await user.click(await screen.findByRole("button", { name: "Delete session" }));
  expect(page.url.pathname).toBe("/");
});
