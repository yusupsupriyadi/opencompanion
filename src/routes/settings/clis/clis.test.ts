import { render, screen } from "@testing-library/svelte";
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
