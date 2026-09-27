import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, onTestFinished, test, vi } from "vitest";
import type { Settings } from "$lib/api";
import { setLang } from "$lib/i18n.svelte";
import { app, initTheme, toast } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend } from "../../test/fixtures";
import SettingsPage from "./+page.svelte";

const base: Settings = {
  onboarded: true,
  chatCli: "claude",
  notifyWaiting: true,
  notifyDone: true,
  notifyError: true,
  companionEnabled: false,
  companionPort: 8765,
  cliPaths: {},
  cliArgs: {},
  scanSeconds: 10,
  plannerCanRead: true,
  projectRoots: [],
  permissionMode: "ask",
  textSize: 100,
  chatModels: {},
  plannerSource: "cli",
  plannerApi: { baseUrl: "", model: "", apiKey: "" },
  keepDays: 0,
  notifyClis: {},
  notifyProjects: {},
  closeToTray: true,
  trayHintShown: false,
  startAtLogin: false,
  autoRunFolders: [],
  language: "en",
};

let stored: Settings;

function api() {
  stored = { ...base };
  return backend({
    get_settings: () => stored,
    save_settings: (a) => {
      stored = a?.settings as Settings;
      return stored;
    },
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: "C:\\data", counts: { sessions: 0, devices: 0 } }),
    default_project_roots: () => ["C:\\Users\\me\\Project"],
    project_folders: () => [],
  });
}

beforeEach(() => {
  app.clis = CLIS;
});

test("Plan is saved as soon as it is picked", async () => {
  const calls = api();
  render(SettingsPage);
  await userEvent.setup().click(await screen.findByLabelText(/^Plan/));
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { permissionMode: "plan" } });
});

test("Bypass needs a second, explicit press, and Keep undoes the pick", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(SettingsPage);
  await user.click(await screen.findByLabelText(/^Bypass/));
  expect(screen.getByRole("alert")).toHaveTextContent("Turn on Bypass for every new session?");
  expect(calls.calls("save_settings")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Keep Ask me" }));
  expect(screen.getByLabelText(/^Ask me/)).toBeChecked();
  expect(calls.calls("save_settings")).toHaveLength(0);

  await user.click(screen.getByLabelText(/^Bypass/));
  await user.click(screen.getByRole("button", { name: "Use Bypass" }));
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { permissionMode: "bypass" } });
});

test("text size starts at the stored size and is saved as soon as another is picked", async () => {
  const calls = api();
  stored = { ...base, textSize: 110 };
  render(SettingsPage);
  expect(await screen.findByRole("radio", { name: /^110%/ })).toBeChecked();
  expect(screen.getByRole("radio", { name: "100% Default" })).not.toBeChecked();

  await userEvent.setup().click(screen.getByRole("radio", { name: /^125%/ }));
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { textSize: 125 } });
  expect(screen.getByRole("radio", { name: /^125%/ })).toBeChecked();
});

test("a text size that fails to save goes back to the stored one", async () => {
  const calls = backend({
    get_settings: () => base,
    save_settings: () => new Error("disk full"),
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: "C:\\data", counts: { sessions: 0, devices: 0 } }),
    default_project_roots: () => [],
    project_folders: () => [],
  });
  render(SettingsPage);
  await userEvent.setup().click(await screen.findByRole("radio", { name: /^150%/ }));
  expect(calls.calls("save_settings")).toHaveLength(1);
  expect(screen.getByRole("radio", { name: /^100%/ })).toBeChecked();
  expect(screen.getByRole("radio", { name: /^150%/ })).not.toBeChecked();
});

test("the table shows what each mode passes to each CLI", async () => {
  api();
  render(SettingsPage);
  expect(await screen.findByText("--dangerously-skip-permissions")).toBeInTheDocument();
  expect(screen.getByText("--dangerously-bypass-approvals-and-sandbox")).toBeInTheDocument();
  expect(screen.getByText("--agent plan")).toBeInTheDocument();
});

test("a custom provider is picked, then saved with its address, model and key", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(SettingsPage);
  await user.selectOptions(await screen.findByLabelText(/^What turns your chat messages/), "api");
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { plannerSource: "api", chatCli: "claude" } });
  expect(screen.getByRole("checkbox", { name: "Let the planner read my project folders" })).toBeDisabled();

  await user.type(screen.getByLabelText("Base URL"), " http://localhost:11434/v1 ");
  await user.type(screen.getByLabelText("Model"), "qwen3-coder");
  await user.type(screen.getByLabelText("API key"), "sk-local");
  expect(screen.getByLabelText("API key")).toHaveAttribute("type", "password");
  await user.click(screen.getByRole("button", { name: "Show" }));
  expect(screen.getByLabelText("API key")).toHaveAttribute("type", "text");
  await user.click(screen.getByRole("button", { name: "Save provider" }));
  expect(calls.calls("save_settings").at(-1)).toMatchObject({
    settings: { plannerSource: "api", plannerApi: { baseUrl: "http://localhost:11434/v1", model: "qwen3-coder", apiKey: "sk-local" } },
  });
});

test("the provider form names what is missing instead of saving", async () => {
  const calls = api();
  stored = { ...base, plannerSource: "api" };
  const user = userEvent.setup();
  render(SettingsPage);
  await user.click(await screen.findByRole("button", { name: "Save provider" }));
  expect(screen.getByText("Add the provider's base URL.")).toBeInTheDocument();
  expect(screen.getByText("Add the model name.")).toBeInTheDocument();
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("aria-invalid", "true");

  await user.type(screen.getByLabelText("Base URL"), "localhost:11434/v1");
  await user.type(screen.getByLabelText("Model"), "llama3.1");
  await user.click(screen.getByRole("button", { name: "Save provider" }));
  expect(screen.getByText("The base URL must start with http:// or https://.")).toBeInTheDocument();
  expect(calls.calls("save_settings")).toHaveLength(0);
});

test("finished sessions are kept for the time picked, and Delete history refreshes every list", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(SettingsPage);
  const keep = await screen.findByLabelText("Keep finished sessions for");
  expect(keep).toHaveValue("0");
  await user.selectOptions(keep, "30");
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { keepDays: 30 } });

  await user.click(screen.getByRole("button", { name: "Delete finished sessions" }));
  await user.click(screen.getByRole("button", { name: "Press again to delete finished sessions" }));
  expect(calls.calls("delete_history")).toHaveLength(1);
  expect(calls.calls("list_sessions")).toHaveLength(1);
});

test("the start at sign-in is offered where the platform has it, and saved when picked", async () => {
  api();
  backend({
    get_settings: () => stored,
    save_settings: (a) => {
      stored = a?.settings as Settings;
      return stored;
    },
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: String.raw`C:\data`, counts: { sessions: 0, devices: 0 }, canStartAtLogin: true }),
    default_project_roots: () => [],
    project_folders: () => [],
  });
  const user = userEvent.setup();
  render(SettingsPage);
  const start = await screen.findByRole("checkbox", { name: "Start in the tray when I sign in to Windows" });
  expect(start).not.toBeChecked();
  await user.click(start);
  expect(stored.startAtLogin).toBe(true);
});

test("closing the window keeps the app in the tray until that is turned off", async () => {
  const calls = api();
  const user = userEvent.setup();
  render(SettingsPage);
  const tray = await screen.findByRole("checkbox", { name: "Keep running in the tray" });
  expect(tray).toBeChecked();
  expect(screen.getByText(/Sessions keep running and your phone can still reach them/)).toBeInTheDocument();
  await user.click(tray);
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { closeToTray: false } });
  expect(screen.getByText(/Closing the window quits OpenCompanion and stops every session it started/)).toBeInTheDocument();
});

test("a setting that could not be saved shows the stored value again", async () => {
  api();
  backend({
    get_settings: () => stored,
    save_settings: () => new Error("The database is locked."),
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: String.raw`C:\data`, counts: { sessions: 0, devices: 0 } }),
    default_project_roots: () => [],
    project_folders: () => [],
  });
  const user = userEvent.setup();
  render(SettingsPage);
  const waiting = await screen.findByRole("checkbox", { name: "A session is waiting for you" });
  expect(waiting).toBeChecked();
  await user.click(waiting);
  await vi.waitFor(() => expect(waiting).toBeChecked());

  const scan = screen.getByLabelText(/How often OpenCompanion looks/);
  await user.selectOptions(scan, "60");
  await vi.waitFor(() => expect(scan).toHaveValue("10"));
  expect(toast.text).toBe("The database is locked.");
});

test("each CLI and each project folder can turn off some notifications", async () => {
  const uninote = String.raw`C:\Users\me\Project\uninote`;
  api();
  backend({
    get_settings: () => stored,
    save_settings: (a) => {
      stored = a?.settings as Settings;
      return stored;
    },
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: String.raw`C:\data`, counts: { sessions: 0, devices: 0 } }),
    default_project_roots: () => [],
    project_folders: () => [{ path: uninote, name: "uninote", markers: [], source: "recent" }],
  });
  const user = userEvent.setup();
  render(SettingsPage);

  const codexDone = await screen.findByRole("checkbox", { name: "Codex CLI: Done" });
  await user.click(codexDone);
  expect(stored.notifyClis).toEqual({ codex: { waiting: true, done: false, error: true } });
  await user.click(screen.getByRole("checkbox", { name: "Codex CLI: Done" }));
  expect(stored.notifyClis).toEqual({});

  await user.selectOptions(screen.getByLabelText("Project folder to add a rule for"), uninote);
  await user.click(screen.getByRole("button", { name: "Add notification rule" }));
  expect(stored.notifyProjects).toEqual({ [uninote]: { waiting: true, done: true, error: true } });
  await user.click(screen.getByRole("checkbox", { name: "uninote: Error" }));
  expect(stored.notifyProjects[uninote].error).toBe(false);
  await user.click(screen.getByRole("button", { name: "Remove the rule for uninote" }));
  expect(stored.notifyProjects).toEqual({});
});

test("a folder runs cards without asking only after a second, explicit press", async () => {
  const uninote = String.raw`C:\Users\me\Project\uninote`;
  api();
  backend({
    get_settings: () => stored,
    save_settings: (a) => {
      stored = a?.settings as Settings;
      return stored;
    },
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    list_devices: () => [],
    app_info: () => ({ version: "0.1.0", dataDir: String.raw`C:\data`, counts: { sessions: 0, devices: 0 } }),
    default_project_roots: () => [],
    project_folders: () => [{ path: uninote, name: "uninote", markers: [], source: "recent" }],
  });
  const user = userEvent.setup();
  render(SettingsPage);

  expect(await screen.findByText("None: every card waits for Run.")).toBeInTheDocument();
  const picker = screen.getByLabelText("Folder whose cards may start without asking");
  await within(picker).findByRole("option", { name: /uninote/ });
  await user.selectOptions(picker, uninote);
  await user.click(screen.getByRole("button", { name: "Add auto-run folder" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Let cards for uninote start without Run?");
  await user.click(screen.getByRole("button", { name: "Keep asking" }));
  expect(stored.autoRunFolders).toEqual([]);

  await user.selectOptions(screen.getByLabelText("Folder whose cards may start without asking"), uninote);
  await user.click(screen.getByRole("button", { name: "Add auto-run folder" }));
  await user.click(screen.getByRole("button", { name: "Start them without asking" }));
  expect(stored.autoRunFolders).toEqual([uninote]);
  await user.click(screen.getByRole("button", { name: "Stop running cards without asking in uninote" }));
  expect(stored.autoRunFolders).toEqual([]);
});

test("picking Bahasa Indonesia saves the language and the page then reads in Indonesian", async () => {
  // The language lives in module state, so the next test starts in English again.
  onTestFinished(() => setLang("en"));
  const calls = api();
  const user = userEvent.setup();
  render(SettingsPage);
  await user.selectOptions(await screen.findByLabelText("Show OpenCompanion in"), "id");
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { language: "id" } });

  expect(await screen.findByLabelText("Tampilkan OpenCompanion dalam")).toHaveValue("id");
  expect(screen.getByRole("checkbox", { name: "Tetap berjalan di tray" })).toBeChecked();
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Pengaturan");
  await vi.waitFor(() => expect(toast.text).toBe("OpenCompanion sekarang memakai Bahasa Indonesia."));
});

test("the Settings tabs lead to General, CLIs and Skills, with General marked", async () => {
  api();
  setUrl("/settings");
  render(SettingsPage);
  const tabs = within(screen.getByRole("navigation", { name: "Settings sections" }));
  expect(tabs.getAllByRole("link").map((a) => [a.textContent, a.getAttribute("href")])).toEqual([
    ["General", "/settings"],
    ["CLIs", "/settings/clis"],
    ["Skills", "/settings/skills"],
  ]);
  expect(tabs.getByRole("link", { name: "General" })).toHaveAttribute("aria-current", "page");
  expect(tabs.getByRole("link", { name: "CLIs" })).not.toHaveAttribute("aria-current");
  // Phone access sits here now that the sidebar no longer links to it.
  expect(await screen.findByRole("switch", { name: "Phone access" })).toBeInTheDocument();
});

test("Day and Dusk switch the theme and show which one is on", async () => {
  api();
  initTheme();
  const user = userEvent.setup();
  render(SettingsPage);
  const dusk = await screen.findByRole("radio", { name: "Dusk" });
  await user.click(dusk);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(dusk).toBeChecked();
  await user.click(screen.getByRole("radio", { name: "Day" }));
  expect(document.documentElement.dataset.theme).toBe("light");
  expect(dusk).not.toBeChecked();
});
