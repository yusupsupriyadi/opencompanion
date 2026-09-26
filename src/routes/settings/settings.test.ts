import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { Settings } from "$lib/api";
import { app } from "$lib/store.svelte";
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
