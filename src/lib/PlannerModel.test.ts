import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import { CLIS, backend } from "../test/fixtures";
import type { ChatModel, CliInstall, ModelList, Settings } from "./api";
import PlannerModel from "./PlannerModel.svelte";
import { app, forgetModelLists, toast } from "./store.svelte";

const claude = CLIS[0];
const opencode: CliInstall = { ...CLIS[2], path: "C:\\nvm4w\\opencode.exe", version: "1.18.32" };

const CLAUDE_LIST: ModelList = {
  models: [
    { id: "opus", label: "Opus", group: null, efforts: ["low", "medium", "high", "xhigh", "max"] },
    { id: "haiku", label: "Haiku", group: null, efforts: ["low", "medium"] },
  ],
  defaultEfforts: ["low", "medium", "high", "xhigh", "max"],
};

const OPENCODE_LIST: ModelList = {
  models: [
    { id: "opencode/big-pickle", label: "Big Pickle", group: "opencode", efforts: [] },
    { id: "openai/gpt-5.5", label: "GPT-5.5", group: "openai", efforts: ["none", "low", "high"] },
  ],
  defaultEfforts: [],
};

function withSaved(chatModels: Record<string, ChatModel>) {
  app.settings = { chatModels } as Settings;
}

function models(list: ModelList | Error, save?: (a: Record<string, unknown> | undefined) => unknown) {
  return backend({
    chat_models: () => list,
    chat_set_model: save ?? ((a) => ({ chatModels: { [a?.cli as string]: { model: a?.model, effort: a?.effort } } })),
  });
}

const modelSelect = () => screen.getByLabelText("Model") as HTMLSelectElement;
const effortSelect = () => screen.getByLabelText("Thinking") as HTMLSelectElement;

beforeEach(() => {
  forgetModelLists();
  app.settings = null;
  toast.show = false;
});

test("shows the saved model and level, and saves a new model with the level kept", async () => {
  withSaved({ claude: { model: "haiku", effort: "medium" } });
  const api = models(CLAUDE_LIST);
  render(PlannerModel, { cli: claude });
  expect(await screen.findByRole("option", { name: "Opus" })).toBeInTheDocument();
  expect(modelSelect().value).toBe("haiku");
  expect(effortSelect().value).toBe("medium");

  await userEvent.setup().selectOptions(modelSelect(), "opus");
  expect(api.calls("chat_set_model")).toEqual([{ cli: "claude", model: "opus", effort: "medium" }]);
  expect(app.settings?.chatModels.claude).toEqual({ model: "opus", effort: "medium" });
  expect(within(effortSelect()).getAllByRole("option").map((o) => o.textContent)).toEqual([
    "CLI default",
    "Low",
    "Medium",
    "High",
    "Extra high",
    "Max",
  ]);
});

test("a level the new model does not take goes back to the CLI default", async () => {
  withSaved({ claude: { model: "opus", effort: "max" } });
  const api = models(CLAUDE_LIST);
  render(PlannerModel, { cli: claude });
  await screen.findByRole("option", { name: "Haiku" });
  await userEvent.setup().selectOptions(modelSelect(), "haiku");
  expect(api.calls("chat_set_model")).toEqual([{ cli: "claude", model: "haiku", effort: "" }]);
  expect(effortSelect().value).toBe("");
});

test("picking a thinking level saves it for the CLI default model", async () => {
  const api = models(CLAUDE_LIST);
  render(PlannerModel, { cli: claude });
  await screen.findByRole("option", { name: "Opus" });
  expect(modelSelect().value).toBe("");
  await userEvent.setup().selectOptions(effortSelect(), "high");
  expect(api.calls("chat_set_model")).toEqual([{ cli: "claude", model: "", effort: "high" }]);
});

test("OpenCode models are grouped by provider and thinking follows the model's variants", async () => {
  models(OPENCODE_LIST);
  render(PlannerModel, { cli: opencode });
  const group = await screen.findByRole("group", { name: "openai" });
  expect(within(group).getByRole("option", { name: "GPT-5.5" })).toBeInTheDocument();
  expect(effortSelect()).toBeDisabled();

  await userEvent.setup().selectOptions(modelSelect(), "openai/gpt-5.5");
  expect(effortSelect()).toBeEnabled();
  expect(within(effortSelect()).getAllByRole("option").map((o) => o.textContent)).toEqual(["CLI default", "None", "Low", "High"]);
});

test("a saved model the list no longer has stays selected and says it is the saved one", async () => {
  withSaved({ claude: { model: "sonnet", effort: "xhigh" } });
  models(CLAUDE_LIST);
  render(PlannerModel, { cli: claude });
  expect(await screen.findByRole("option", { name: "sonnet (saved)" })).toBeInTheDocument();
  expect(modelSelect().value).toBe("sonnet");
  expect(effortSelect().value).toBe("xhigh");
});

test("a list that fails says why, keeps the saved model and can be asked for again", async () => {
  withSaved({ codex: { model: "gpt-5.5", effort: "high" } });
  const codex = CLIS[1];
  const api = models(new Error("Codex CLI could not list its models: not logged in"));
  render(PlannerModel, { cli: codex });
  expect(await screen.findByRole("alert")).toHaveTextContent("not logged in");
  expect(modelSelect().value).toBe("gpt-5.5");
  expect(effortSelect().value).toBe("high");

  await userEvent.setup().click(screen.getByRole("button", { name: "List models again" }));
  expect(api.calls("chat_models")).toEqual([{ cli: "codex" }, { cli: "codex" }]);
});

test("a save that fails puts the stored choice back", async () => {
  withSaved({ claude: { model: "opus", effort: "high" } });
  models(CLAUDE_LIST, () => new Error("database is locked"));
  render(PlannerModel, { cli: claude });
  await screen.findByRole("option", { name: "Haiku" });
  await userEvent.setup().selectOptions(modelSelect(), "haiku");
  expect(modelSelect().value).toBe("opus");
  expect(effortSelect().value).toBe("high");
  expect(toast.text).toBe("The planner model could not be saved: database is locked");
});
