import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { tick } from "svelte";
import { expect, test } from "vitest";
import { goto } from "../test/app-navigation";
import { setUrl } from "../test/app-state.svelte";
import SettingsSidebar from "./SettingsSidebar.svelte";

const nav = () => within(screen.getByRole("navigation", { name: "Settings sections" }));

test("every item points at a Settings screen, grouped under its heading", () => {
  setUrl("/settings");
  render(SettingsSidebar);
  const groups = screen.getAllByRole("list").map((ul) => [
    ul.getAttribute("aria-labelledby") && document.getElementById(ul.getAttribute("aria-labelledby")!)?.textContent,
    within(ul).getAllByRole("link").map((a) => [a.textContent, a.getAttribute("href")]),
  ]);
  expect(groups).toEqual([
    [
      "General",
      [
        ["Phone access", "/settings"],
        ["Window and sign-in", "/settings?s=window"],
        ["Language", "/settings?s=language"],
      ],
    ],
    [
      "Agents",
      [
        ["CLIs", "/settings/clis"],
        ["Skills", "/settings/skills"],
        ["Chat planner", "/settings?s=planner"],
        ["Permission mode", "/settings?s=permissions"],
      ],
    ],
    [
      "Workspace",
      [
        ["Project folders", "/settings?s=projects"],
        ["Notifications", "/settings?s=notifications"],
        ["Outside sessions", "/settings?s=outside"],
        ["History", "/settings?s=history"],
      ],
    ],
    [
      "Appearance",
      [
        ["Theme", "/settings?s=theme"],
        ["Text size", "/settings?s=text-size"],
      ],
    ],
  ]);
});

test("the open screen is marked, whether a section or a route", async () => {
  setUrl("/settings");
  render(SettingsSidebar);
  expect(nav().getByRole("link", { name: "Phone access" })).toHaveAttribute("aria-current", "page");

  setUrl("/settings?s=theme");
  await tick();
  expect(nav().getByRole("link", { name: "Theme" })).toHaveAttribute("aria-current", "page");
  expect(nav().getByRole("link", { name: "Phone access" })).not.toHaveAttribute("aria-current");

  setUrl("/settings/clis");
  await tick();
  expect(nav().getByRole("link", { name: "CLIs" })).toHaveAttribute("aria-current", "page");
  expect(nav().getAllByRole("link").filter((a) => a.hasAttribute("aria-current"))).toHaveLength(1);
});

test("Back returns to the app screen open before Settings", () => {
  setUrl("/settings");
  render(SettingsSidebar, { back: "/session?id=abc" });
  expect(screen.getByRole("link", { name: "Back to app" })).toHaveAttribute("href", "/session?id=abc");
});

test("search keeps the items whose name or text matches, and Escape brings them all back", async () => {
  setUrl("/settings");
  const user = userEvent.setup();
  render(SettingsSidebar);
  const box = screen.getByRole("searchbox", { name: "Search settings" });

  await user.type(box, "tray");
  expect(nav().getAllByRole("link").map((a) => a.textContent)).toEqual(["Window and sign-in"]);
  expect(screen.getByText("General")).toBeInTheDocument();
  expect(screen.queryByText("Agents")).toBeNull();

  await user.keyboard("{Escape}");
  expect(box).toHaveValue("");
  expect(nav().getAllByRole("link")).toHaveLength(13);
});

test("a search with no match says so, and Enter opens the first match", async () => {
  setUrl("/settings");
  const user = userEvent.setup();
  render(SettingsSidebar);
  const box = screen.getByRole("searchbox", { name: "Search settings" });

  await user.type(box, "zzz");
  expect(screen.getByRole("status")).toHaveTextContent("No setting matches “zzz”. Try another word.");

  await user.clear(box);
  await user.type(box, "dusk{Enter}");
  expect(goto).toHaveBeenLastCalledWith("/settings?s=theme");
});

test("Ctrl+F jumps to the search", async () => {
  setUrl("/settings");
  const user = userEvent.setup();
  render(SettingsSidebar);
  await user.keyboard("{Control>}f{/Control}");
  expect(screen.getByRole("searchbox", { name: "Search settings" })).toHaveFocus();
});
