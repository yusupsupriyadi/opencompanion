import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { CLIS, backend } from "../test/fixtures";
import SessionForm from "./SessionForm.svelte";
import { app } from "./store.svelte";

beforeEach(() => {
  app.clis = CLIS;
  app.clisState = "ready";
});

function setup(onsubmit = vi.fn(async () => undefined)) {
  render(SessionForm, { title: "New session", initial: {}, onsubmit, oncancel: vi.fn() });
  return { user: userEvent.setup(), onsubmit };
}

test("a folder that does not exist is refused before anything starts", async () => {
  const api = backend({ recent_projects: () => [], folder_exists: () => false });
  const { user, onsubmit } = setup();
  await user.type(screen.getByLabelText("Project folder"), "C:\\nowhere\\app");
  await user.click(screen.getByRole("button", { name: "Start Claude Code in app" }));
  expect(await screen.findByText("This folder does not exist.")).toBeInTheDocument();
  expect(screen.getByLabelText("Project folder")).toHaveAttribute("aria-invalid", "true");
  expect(api.calls("folder_exists")).toEqual([{ path: "C:\\nowhere\\app" }]);
  expect(onsubmit).not.toHaveBeenCalled();
});

test("headless needs a prompt, then the chosen values are submitted", async () => {
  backend({ recent_projects: () => [{ path: "C:\\p\\uninote", lastUsed: 1 }], folder_exists: () => true });
  const { user, onsubmit } = setup();
  await user.click(screen.getByLabelText(/Codex CLI/));
  await user.click(await screen.findByRole("button", { name: "uninote" }));
  await user.click(screen.getByLabelText(/Headless/));
  expect(screen.getByLabelText("Prompt")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Start Codex CLI in uninote" }));
  expect(await screen.findByText("Headless sessions need a prompt.")).toBeInTheDocument();
  expect(onsubmit).not.toHaveBeenCalled();

  await user.type(screen.getByLabelText("Prompt"), "Fix the failing tests");
  await user.click(screen.getByRole("button", { name: "Start Codex CLI in uninote" }));
  expect(onsubmit).toHaveBeenCalledWith({ cli: "codex", cwd: "C:\\p\\uninote", mode: "headless", prompt: "Fix the failing tests", permissionMode: "ask" });
});

test("CLIs that are not installed cannot be picked", () => {
  backend({ recent_projects: () => [] });
  setup();
  expect(screen.getByLabelText(/OpenCode/)).toBeDisabled();
  expect(screen.getByLabelText(/Gemini CLI/)).toBeDisabled();
  expect(screen.getByLabelText(/Claude Code/)).toBeChecked();
});

test("Browse fills the folder from the system picker", async () => {
  backend({ recent_projects: () => [] });
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "Browse" }));
  expect(screen.getByLabelText("Project folder")).toHaveValue("C:\\Users\\me\\Project\\picked");
});

test("the permission mode starts from Settings and Bypass is called out", async () => {
  backend({ recent_projects: () => [], folder_exists: () => true });
  app.settings = { permissionMode: "plan" } as typeof app.settings;
  const { user, onsubmit } = setup();
  const select = screen.getByLabelText("Permission mode");
  expect(select).toHaveValue("plan");
  expect(screen.getByText(/Reads and plans, changes nothing/)).toBeInTheDocument();
  await user.selectOptions(select, "bypass");
  expect(screen.getByRole("note")).toHaveTextContent("Bypass: Claude Code can change or delete any file");
  await user.type(screen.getByLabelText("Project folder"), "C:\\p\\x");
  await user.click(screen.getByRole("button", { name: "Start Claude Code in x" }));
  expect(onsubmit).toHaveBeenCalledWith(expect.objectContaining({ permissionMode: "bypass" }));
  app.settings = null;
});

test("a start error from the backend is shown in the form", async () => {
  backend({ recent_projects: () => [], folder_exists: () => true });
  const { user } = setup(vi.fn(async () => Promise.reject("Claude Code is not installed or not on PATH.")));
  await user.type(screen.getByLabelText("Project folder"), "C:\\p\\x");
  await user.click(screen.getByRole("button", { name: "Start Claude Code in x" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Claude Code is not installed or not on PATH.");
});
