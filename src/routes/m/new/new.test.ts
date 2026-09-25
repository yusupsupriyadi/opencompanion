import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { CliInstall, Task } from "$lib/api";
import { setUrl } from "../../../test/app-state.svelte";
import { phoneServer, sent } from "../../../test/phone-server";
import NewSession from "./+page.svelte";

const clis: CliInstall[] = [
  { kind: "claude", label: "Claude Code", path: "C:\\bin\\claude.exe", version: "2.1.282", tested: true, error: null },
  { kind: "codex", label: "Codex CLI", path: null, version: null, tested: true, error: null },
];
const folders = [{ path: "C:\\Users\\me\\Project\\uninote", name: "uninote", markers: [], source: "recent" }];
const options = () => [200, { clis, folders, permissionMode: "auto" }] as [number, unknown];

beforeEach(() => {
  vi.mocked(goto).mockClear();
});

test("a headless session needs a prompt, then starts in the chosen folder", async () => {
  setUrl("/m/new");
  const fetchMock = phoneServer({
    "GET /api/options": options,
    "POST /api/sessions": () => [200, { session: { id: "s1" } }],
  });
  const user = userEvent.setup();
  render(NewSession);
  const start = await screen.findByRole("button", { name: "Start Claude Code in uninote" });
  expect(screen.getByRole("radio", { name: /Codex CLI/ })).toBeDisabled();
  expect(screen.getByLabelText("Permission mode")).toHaveValue("auto");

  await user.click(start);
  expect(screen.getByText("Headless sessions need a prompt.")).toBeInTheDocument();
  expect(sent(fetchMock, "POST /api/sessions")).toHaveLength(0);

  await user.type(screen.getByLabelText("Prompt"), "fix the failing tests");
  await user.click(start);
  expect(sent(fetchMock, "POST /api/sessions")).toEqual([
    { cli: "claude", cwd: "C:\\Users\\me\\Project\\uninote", mode: "headless", prompt: "fix the failing tests", permissionMode: "auto" },
  ]);
  expect(goto).toHaveBeenCalledWith("/m/session?id=s1", { replaceState: true });
});

test("a folder the desktop cannot find is flagged at the folder field", async () => {
  setUrl("/m/new");
  phoneServer({
    "GET /api/options": options,
    "POST /api/sessions": () => [409, { error: "This folder does not exist." }],
  });
  const user = userEvent.setup();
  render(NewSession);
  await user.selectOptions(await screen.findByLabelText("Project folder"), "Another folder…");
  await user.type(screen.getByLabelText("Folder path"), "D:\\gone");
  await user.click(screen.getByRole("radio", { name: /Interactive/ }));
  await user.click(screen.getByRole("button", { name: "Start Claude Code in gone" }));
  expect(await screen.findByText("This folder does not exist.")).toBeInTheDocument();
  expect(screen.getByLabelText("Folder path")).toHaveAttribute("aria-invalid", "true");
  expect(goto).not.toHaveBeenCalled();
});

test("running a Board card starts from the card and runs it through the Board", async () => {
  setUrl("/m/new?task=t1");
  const task: Task = {
    id: "t1",
    title: "Write the API docs",
    notes: "Document every endpoint",
    project: "C:\\Work\\api",
    cli: "claude",
    column: "todo",
    position: 1,
    sessionId: null,
    createdAt: 1,
    updatedAt: 1,
  };
  const fetchMock = phoneServer({
    "GET /api/options": options,
    "GET /api/tasks": () => [200, { tasks: [task] }],
    "POST /api/tasks/t1/run": () => [200, { session: { id: "s9" } }],
  });
  const user = userEvent.setup();
  render(NewSession);
  expect(await screen.findByRole("heading", { name: "Write the API docs" })).toBeInTheDocument();
  // A folder outside the found list is kept as a typed path.
  expect(screen.getByLabelText("Folder path")).toHaveValue("C:\\Work\\api");
  expect(screen.getByLabelText("Prompt")).toHaveValue("Document every endpoint");
  await user.click(screen.getByRole("button", { name: "Run in api" }));
  expect(sent(fetchMock, "POST /api/tasks/t1/run")[0]).toMatchObject({ cli: "claude", cwd: "C:\\Work\\api", prompt: "Document every endpoint" });
  expect(goto).toHaveBeenCalledWith("/m/session?id=s9", { replaceState: true });
});
