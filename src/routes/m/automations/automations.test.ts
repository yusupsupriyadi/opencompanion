import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { Automation, AutomationView, CliInstall } from "$lib/api";
import { receive } from "$lib/phone.svelte";
import { setUrl } from "../../../test/app-state.svelte";
import { phoneServer, sent } from "../../../test/phone-server";
import Edit from "./edit/+page.svelte";
import List from "./+page.svelte";

const DAY = 86_400_000;
const clis: CliInstall[] = [
  { kind: "claude", label: "Claude Code", path: "C:\\bin\\claude.exe", version: "2.1.282", tested: true, error: null },
  { kind: "codex", label: "Codex CLI", path: null, version: null, tested: true, error: null },
];
const folders = [{ path: "C:\\Users\\me\\Project\\uninote", name: "uninote", markers: [], source: "recent" }];
const options = () => [200, { clis, folders, permissionMode: "auto" }] as [number, unknown];

function automation(over: Partial<Automation> = {}): Automation {
  return {
    id: "a1",
    name: "Review pull requests",
    cli: "claude",
    cwd: "C:\\Users\\me\\Project\\uninote",
    mode: "headless",
    prompt: "review the open pull requests",
    permissionMode: "ask",
    schedule: "0 9 * * 1-5",
    enabled: true,
    nextRunAt: Date.now() + DAY,
    createdAt: 1,
    updatedAt: 1,
    ...over,
  };
}

beforeEach(() => {
  vi.mocked(goto).mockClear();
});

test("the phone lists automations, and a switch pauses one", async () => {
  setUrl("/m/automations");
  let rows: AutomationView[] = [{ ...automation(), lastRun: null }];
  const fetchMock = phoneServer({
    "GET /api/automations": () => [200, { automations: rows }],
    "POST /api/automations/a1/enabled": (body) => [200, { automation: automation({ enabled: body?.enabled as boolean, nextRunAt: null }) }],
  });
  const user = userEvent.setup();
  render(List);
  const card = await screen.findByRole("link", { name: /Review pull requests/ });
  expect(card).toHaveAttribute("href", "/m/automations/edit?id=a1");
  expect(card).toHaveTextContent("Weekdays at 09:00 · uninote");
  expect(screen.getByRole("link", { name: "New automation" })).toHaveAttribute("href", "/m/automations/edit?new");

  await user.click(screen.getByRole("switch", { name: "Review pull requests is active" }));
  expect(sent(fetchMock, "POST /api/automations/a1/enabled")).toEqual([{ enabled: false }]);

  // A change on the desktop reaches the phone through the socket.
  rows = [{ ...automation({ name: "Nightly tests", enabled: false, nextRunAt: null }), lastRun: null }];
  receive({ type: "automations" });
  const paused = await screen.findByRole("link", { name: /Nightly tests/ });
  expect(within(paused).getByText("Paused")).toBeInTheDocument();
});

test("with no automation the phone says what one does", async () => {
  setUrl("/m/automations");
  phoneServer({ "GET /api/automations": () => [200, { automations: [] }] });
  render(List);
  expect(await screen.findByText(/No automations yet\./)).toBeInTheDocument();
});

test("a new automation is sent with its schedule as cron", async () => {
  setUrl("/m/automations/edit?new");
  const fetchMock = phoneServer({
    "GET /api/options": options,
    "POST /api/automations/preview": () => [200, { times: [Date.now() + DAY] }],
    "POST /api/automations": () => [200, { automation: automation({ id: "n1", name: "Hourly check" }) }],
  });
  const user = userEvent.setup();
  render(Edit);
  const save = await screen.findByRole("button", { name: "Create automation" });
  expect(screen.getByLabelText("Permission mode")).toHaveValue("auto");

  await user.click(save);
  expect(screen.getByText("Give the automation a name.")).toBeInTheDocument();

  await user.type(screen.getByLabelText("Name"), "Hourly check");
  await user.type(screen.getByLabelText("Prompt"), "run the tests and report failures");
  await user.click(screen.getByLabelText("Every"));
  await user.selectOptions(screen.getAllByRole("combobox", { name: "How often" })[0], "2");
  await user.click(save);
  expect(sent(fetchMock, "POST /api/automations")).toEqual([
    {
      name: "Hourly check",
      cli: "claude",
      cwd: "C:\\Users\\me\\Project\\uninote",
      mode: "headless",
      prompt: "run the tests and report failures",
      permissionMode: "auto",
      schedule: "0 */2 * * *",
      enabled: true,
    },
  ]);
  expect(goto).toHaveBeenCalledWith("/m/automations", { replaceState: true });
});

test("an automation opens filled in, saves changes, runs now and is deleted after the sheet", async () => {
  setUrl("/m/automations/edit?id=a1");
  const detail = {
    automation: automation({ cwd: "D:\\work\\api", schedule: "15 18 * * 1,3" }),
    runs: [{ id: "r1", automationId: "a1", dueAt: Date.now() - DAY, ranAt: Date.now() - DAY, outcome: "started", sessionId: "s9", error: null }],
  };
  const fetchMock = phoneServer({
    "GET /api/options": options,
    "GET /api/automations/a1": () => [200, detail],
    "POST /api/automations/preview": () => [200, { times: [Date.now() + DAY] }],
    "PUT /api/automations/a1": () => [200, { automation: detail.automation }],
    "POST /api/automations/a1/run": () => [200, { run: { ...detail.runs[0], id: "r2" } }],
    "DELETE /api/automations/a1": () => [200, { ok: true }],
  });
  const user = userEvent.setup();
  render(Edit);
  expect(await screen.findByLabelText("Name")).toHaveValue("Review pull requests");
  // A folder the desktop did not list opens as a typed path.
  expect(screen.getByLabelText("Folder path")).toHaveValue("D:\\work\\api");
  expect(screen.getByLabelText("Time")).toHaveValue("18:15");
  expect(screen.getByRole("button", { name: "Monday" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Tuesday" })).toHaveAttribute("aria-pressed", "false");
  expect(screen.getByRole("link", { name: "Open session" })).toHaveAttribute("href", "/m/session?id=s9");

  await user.click(screen.getByRole("button", { name: "Friday" }));
  await user.click(screen.getByRole("button", { name: "Save automation" }));
  expect(sent(fetchMock, "PUT /api/automations/a1")[0]).toMatchObject({ cwd: "D:\\work\\api", schedule: "15 18 * * 1,3,5" });

  await user.click(screen.getByRole("button", { name: "Run now" }));
  expect(sent(fetchMock, "POST /api/automations/a1/run")).toHaveLength(1);

  await user.click(screen.getByRole("button", { name: "Delete automation" }));
  const sheet = await screen.findByRole("dialog", { name: "Delete this automation?" });
  await user.click(within(sheet).getByRole("button", { name: "Delete automation" }));
  expect(sent(fetchMock, "DELETE /api/automations/a1")).toHaveLength(1);
  expect(goto).toHaveBeenCalledWith("/m/automations", { replaceState: true });
});
