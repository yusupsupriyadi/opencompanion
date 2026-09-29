import { fireEvent, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { goto } from "$app/navigation";
import type { Automation, AutomationRun, AutomationView } from "$lib/api";
import ContextMenu from "$lib/ContextMenu.svelte";
import { closeMenu } from "$lib/context-menu.svelte";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend } from "../../test/fixtures";
import Automations from "./+page.svelte";

const DAY = 86_400_000;

function automation(over: Partial<Automation> = {}): Automation {
  return {
    id: "a1",
    name: "Review pull requests",
    cli: "claude",
    cwd: String.raw`C:\Users\me\Project\uninote`,
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

function run(over: Partial<AutomationRun> = {}): AutomationRun {
  return { id: "r1", automationId: "a1", dueAt: Date.now() - DAY, ranAt: Date.now() - DAY, outcome: "started", sessionId: "s1", error: null, ...over };
}

function tomorrowAt9() {
  const d = new Date();
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1, 9, 0).getTime();
}

const view = (a: Automation, lastRun: AutomationRun | null = null): AutomationView => ({ ...a, lastRun });

beforeEach(() => {
  closeMenu(false);
  app.clis = CLIS;
  app.clisState = "ready";
  app.settings = null;
  app.now = Date.now();
  vi.mocked(goto).mockClear();
  setUrl("/automations");
});

test("with no automation the screen says what one does", async () => {
  backend({ list_automations: () => [], app_info: () => ({ canStartAtLogin: false }) });
  render(Automations);
  expect(await screen.findByText(/No automations yet\./)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "New automation" })).toHaveAttribute("href", "/automations?new");
});

test("a row shows the schedule, the next run and the last run, and its switch pauses it", async () => {
  const calls = backend({
    list_automations: () => [view(automation({ nextRunAt: tomorrowAt9() }), run({ outcome: "late" }))],
    set_automation_enabled: (a) => automation({ enabled: a?.enabled as boolean, nextRunAt: null }),
  });
  render(Automations);
  const row = await screen.findByRole("link", { name: /Review pull requests/ });
  expect(row).toHaveAttribute("href", "/automations?id=a1");
  expect(row).toHaveTextContent("Claude Code · uninote · Weekdays at 09:00");
  expect(row).toHaveTextContent("Next run: tomorrow 09:00");
  expect(within(row).getByText("Late")).toBeInTheDocument();

  const toggle = screen.getByRole("switch", { name: "Review pull requests is active" });
  expect(toggle).toHaveAttribute("aria-checked", "true");
  await userEvent.setup().click(toggle);
  expect(calls.calls("set_automation_enabled")).toEqual([{ id: "a1", enabled: false }]);
});

test("the right-click menu runs an automation now and says what happened", async () => {
  const calls = backend({
    list_automations: () => [view(automation())],
    run_automation: () => run({ outcome: "skipped", sessionId: null }),
  });
  render(ContextMenu);
  render(Automations);
  const row = await screen.findByRole("link", { name: /Review pull requests/ });
  await fireEvent.contextMenu(row);
  const menu = screen.getByRole("menu", { name: "Automation: Review pull requests" });
  const items = within(menu).getAllByRole("menuitem").map((i) => i.textContent?.trim());
  expect(items).toEqual(["Run now", "Edit", "Pause", "Delete automation"]);
  await userEvent.setup().click(within(menu).getByRole("menuitem", { name: "Run now" }));
  expect(calls.calls("run_automation")).toEqual([{ id: "a1" }]);
});

test("a list that cannot be read offers to try again", async () => {
  let fail = true;
  backend({ list_automations: () => (fail ? new Error("database is locked") : []) });
  render(Automations);
  expect(await screen.findByText("The automations could not be read")).toBeInTheDocument();
  fail = false;
  await userEvent.setup().click(screen.getByRole("button", { name: "Try again" }));
  expect(await screen.findByText(/No automations yet\./)).toBeInTheDocument();
});

test("start at sign-in off is pointed out, with the way to turn it on", async () => {
  app.settings = { startAtLogin: false } as typeof app.settings;
  backend({ list_automations: () => [], app_info: () => ({ canStartAtLogin: true }) });
  render(Automations);
  expect(await screen.findByText(/Start at sign-in is off/)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Turn it on in Settings" })).toHaveAttribute("href", "/settings?s=window");
});

test("a new automation needs a name and a day, then saves the schedule as cron", async () => {
  setUrl("/automations?new");
  const calls = backend({
    recent_projects: () => [{ path: String.raw`C:\p\uninote`, lastUsed: 1 }],
    folder_exists: () => true,
    preview_schedule: () => [Date.now() + DAY],
    save_automation: () => automation({ id: "new1" }),
  });
  const user = userEvent.setup();
  render(Automations);
  const create = screen.getByRole("button", { name: "Create automation" });

  await user.click(create);
  expect(screen.getByText("Give the automation a name.")).toBeInTheDocument();
  expect(calls.calls("save_automation")).toHaveLength(0);

  await user.type(screen.getByLabelText("Name"), "Morning review");
  await user.click(await screen.findByRole("button", { name: "uninote" }));
  await user.type(screen.getByLabelText("Prompt"), "review the open pull requests");
  await user.click(screen.getByLabelText("Some days"));
  for (const day of ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]) await user.click(screen.getByRole("button", { name: day }));
  expect(screen.getByText("Pick at least one day.")).toBeInTheDocument();
  await user.click(create);
  expect(calls.calls("save_automation")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Saturday" }));
  expect(screen.queryByText("Pick at least one day.")).not.toBeInTheDocument();
  expect(await screen.findByText(/Next runs: /)).toBeInTheDocument();
  await user.click(create);
  await vi.waitFor(() => expect(calls.calls("save_automation")).toHaveLength(1));
  expect(calls.calls("save_automation")[0]).toEqual({
    id: null,
    draft: {
      name: "Morning review",
      cli: "claude",
      cwd: String.raw`C:\p\uninote`,
      mode: "headless",
      prompt: "review the open pull requests",
      permissionMode: "ask",
      schedule: "0 9 * * 6",
      enabled: true,
    },
  });
  expect(goto).toHaveBeenCalledWith("/automations?id=new1");
});

test("a cron expression the desktop refuses is explained under the field", async () => {
  setUrl("/automations?new");
  backend({
    recent_projects: () => [],
    preview_schedule: (a) => (a?.schedule === "0 25 * * *" ? new Error("This schedule never comes round.") : [Date.now() + DAY]),
  });
  const user = userEvent.setup();
  render(Automations);
  await user.click(screen.getByLabelText("Cron"));
  const field = screen.getByLabelText("Cron expression");
  expect(field).toHaveValue("0 9 * * *");
  await user.clear(field);
  await user.type(field, "0 25 * * *");
  expect(await screen.findByText("This schedule never comes round.")).toBeInTheDocument();
  expect(field).toHaveAttribute("aria-invalid", "true");
});

test("one automation shows its runs, runs now and is deleted after asking", async () => {
  setUrl("/automations?id=a1");
  const calls = backend({
    get_automation: () => ({
      automation: automation({ schedule: "30 7 * * *" }),
      runs: [run({ id: "r2", sessionId: null }), run({ id: "r1", outcome: "failed", sessionId: null, error: "Claude Code is not installed or not on PATH." }), run({ id: "r0" })],
    }),
    recent_projects: () => [],
    preview_schedule: () => [Date.now() + DAY],
    run_automation: () => run({ id: "r3" }),
    delete_automation: () => undefined,
    // After the delete the screen goes back to the list.
    list_automations: () => [],
  });
  const user = userEvent.setup();
  render(Automations);
  expect(await screen.findByRole("heading", { level: 1, name: "Review pull requests" })).toBeInTheDocument();
  expect(screen.getByText(/Every day at 07:30 · Next run:/)).toBeInTheDocument();
  expect(screen.getByLabelText("Name")).toHaveValue("Review pull requests");
  expect(screen.getByLabelText("Time")).toHaveValue("07:30");

  const runs = screen.getByRole("region", { name: "Recent runs" });
  expect(within(runs).getByText("Session deleted")).toBeInTheDocument();
  expect(within(runs).getByText("Claude Code is not installed or not on PATH.")).toBeInTheDocument();
  expect(within(runs).getAllByRole("link", { name: "Open session" }).map((a) => a.getAttribute("href"))).toEqual(["/session?id=s1"]);

  await user.click(screen.getByRole("button", { name: "Run now" }));
  expect(calls.calls("run_automation")).toEqual([{ id: "a1" }]);

  await user.click(screen.getByRole("button", { name: "Delete automation" }));
  const dialog = await screen.findByRole("dialog", { name: "Delete this automation?" });
  await user.click(within(dialog).getByRole("button", { name: "Delete automation" }));
  expect(calls.calls("delete_automation")).toEqual([{ id: "a1" }]);
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/automations"));
});

test("an automation that was deleted says so, with the way back", async () => {
  setUrl("/automations?id=gone");
  backend({ get_automation: () => new Error("This automation was deleted.") });
  render(Automations);
  expect(await screen.findByRole("heading", { name: "This automation was deleted." })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "All automations" })).toHaveAttribute("href", "/automations");
});
