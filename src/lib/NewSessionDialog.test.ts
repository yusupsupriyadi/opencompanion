import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import { setUrl } from "../test/app-state.svelte";
import { CLIS, backend, session } from "../test/fixtures";
import NewSessionDialog from "./NewSessionDialog.svelte";
import { app, askNewSession, pendingNew } from "./store.svelte";

beforeEach(() => {
  app.clis = CLIS;
  app.clisState = "ready";
  pendingNew.open = false;
  vi.mocked(goto).mockClear();
  setUrl("/board");
});

test("opened from a folder, the folder is filled in and the session starts there", async () => {
  const api = backend({
    recent_projects: () => [],
    folder_exists: () => true,
    start_session: () => session({ id: "n1", cwd: "C:\\p\\uninote" }),
  });
  const user = userEvent.setup();
  render(NewSessionDialog);
  askNewSession("C:\\p\\uninote");
  expect(await screen.findByRole("heading", { name: "New session" })).toBeInTheDocument();
  expect(screen.getByLabelText("Project folder")).toHaveValue("C:\\p\\uninote");

  await user.click(screen.getByRole("button", { name: "Start Claude Code in uninote" }));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/session?id=n1"));
  expect(api.calls("start_session")).toEqual([{ req: expect.objectContaining({ cwd: "C:\\p\\uninote", source: "manual" }) }]);
  expect(pendingNew.open).toBe(false);
});

test("opened without a folder, you choose one", async () => {
  backend({ recent_projects: () => [] });
  const user = userEvent.setup();
  render(NewSessionDialog);
  askNewSession();
  expect(await screen.findByRole("heading", { name: "New session" })).toBeInTheDocument();
  expect(screen.getByLabelText("Project folder")).toHaveValue("");

  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.queryByRole("heading", { name: "New session" })).not.toBeInTheDocument();
  expect(pendingNew.open).toBe(false);
});
