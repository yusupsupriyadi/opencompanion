import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import { app } from "$lib/store.svelte";
import { CLIS, backend } from "../../test/fixtures";
import Onboarding from "./+page.svelte";

beforeEach(() => {
  app.clis = CLIS;
  app.clisState = "ready";
  app.settings = null;
});

test("the planner picker starts on the first CLI that can plan, and Continue saves the pick", async () => {
  const calls = backend({
    get_settings: () => ({ onboarded: false, chatCli: null }),
    companion_status: () => ({ running: false, address: null, port: 8765, error: null }),
    save_settings: (a) => a?.settings,
  });
  const user = userEvent.setup();
  render(Onboarding);
  const picker = screen.getByLabelText("Planner for Chat");
  expect(picker).toHaveValue("claude");
  expect(screen.getByRole("button", { name: "Continue with Claude Code" })).toBeInTheDocument();

  await user.selectOptions(picker, "codex");
  await user.click(screen.getByRole("button", { name: "Continue with Codex CLI" }));
  expect(calls.calls("save_settings").at(-1)).toMatchObject({ settings: { onboarded: true, chatCli: "codex" } });
});
