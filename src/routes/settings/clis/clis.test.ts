import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../../test/app-state.svelte";
import { CLIS } from "../../../test/fixtures";
import ClisPage from "./+page.svelte";

test("CLIs is a tab of Settings, marked as the open one", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  render(ClisPage);
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Settings");
  expect(screen.getByRole("link", { name: "CLIs" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("link", { name: "General" })).toHaveAttribute("href", "/settings");
  expect(screen.getByRole("button", { name: "Rescan" })).toBeInTheDocument();
  expect(screen.getByRole("table", { name: "Coding CLIs found on this computer" })).toBeInTheDocument();
});

test("CCS shows its install command, and its settings say the profile comes first", async () => {
  const user = userEvent.setup();
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  render(ClisPage);
  expect(screen.getByText("npm install -g @kaitranntt/ccs")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Set a path for CCS" }));
  expect(screen.getByLabelText("Extra arguments")).toHaveAttribute("placeholder", "For example work --effort high");
  expect(screen.getByText(/Placed right after ccs, so the profile comes first/)).toBeInTheDocument();
});

test("Pi shows its install command, and once found, its JSON mode and the planner button", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  const { unmount } = render(ClisPage);
  expect(screen.getByText("npm install -g --ignore-scripts @earendil-works/pi-coding-agent")).toBeInTheDocument();
  unmount();
  app.clis = CLIS.map((c) => (c.kind === "pi" ? { ...c, path: String.raw`C:\Users\me\.pi\agent\bin\pi.cmd`, version: "0.87.1", tested: true } : c));
  render(ClisPage);
  const row = screen.getByText("Pi", { selector: "b" }).closest("tr")!;
  expect(row).toHaveTextContent("pi --mode json");
  expect(row).toHaveTextContent("0.87.1");
  expect(within(row).getByRole("button", { name: "Use as planner" })).toBeInTheDocument();
});
