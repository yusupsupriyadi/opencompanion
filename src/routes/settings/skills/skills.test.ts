import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { setUrl } from "../../../test/app-state.svelte";
import { backend } from "../../../test/fixtures";
import { skillScan } from "../../../test/skill-scan";
import SkillsPage from "./+page.svelte";

const H = String.raw`C:\Users\me`;

function rowOf(name: string) {
  return screen.getByRole("button", { name }).closest("tr") as HTMLElement;
}

test("Skills is a Settings screen, named by its H1", async () => {
  setUrl("/settings/skills");
  backend({ scan_skills: () => skillScan() });
  render(SkillsPage);
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Skills");
  await screen.findByRole("button", { name: "antislop" });
  expect(screen.getByRole("button", { name: "Rescan" })).toBeInTheDocument();
});

test("the matrix shows what each folder holds", async () => {
  backend({ scan_skills: () => skillScan() });
  render(SkillsPage);
  await screen.findByRole("button", { name: "antislop" });

  expect(screen.getByText("3 skills in 4 folders · 1 differs · 1 has a problem")).toBeInTheDocument();
  const heads = screen.getAllByRole("columnheader").map((h) => h.textContent?.replace(/\s+/g, " ").trim());
  expect(heads[0]).toBe("Skill");
  expect(heads[3]).toContain("Shared");
  expect(heads[5]).toContain("Folder not found");

  const antislop = within(rowOf("antislop"));
  expect(antislop.getAllByText("Same")).toHaveLength(3);
  expect(antislop.getAllByText("Missing")).toHaveLength(1);
  const neo = within(rowOf("browseros-neo"));
  expect(neo.getByText("2 versions")).toBeInTheDocument();
  expect(neo.getByText("Version A")).toBeInTheDocument();
  expect(neo.getAllByText("Version B")).toHaveLength(2);
  expect(within(rowOf("canvas-design")).getByText("No SKILL.md")).toBeInTheDocument();
});

test("filters and search narrow the list and say when nothing is left", async () => {
  const user = userEvent.setup();
  backend({ scan_skills: () => skillScan() });
  render(SkillsPage);
  await screen.findByRole("button", { name: "antislop" });
  const names = () => screen.queryAllByRole("rowheader").map((h) => within(h).getByRole("button").textContent);

  await user.click(screen.getByRole("button", { name: "Different 1" }));
  expect(screen.getByRole("button", { name: "Different 1" })).toHaveAttribute("aria-pressed", "true");
  expect(names()).toEqual(["browseros-neo"]);
  await user.click(screen.getByRole("button", { name: "Problems 1" }));
  expect(names()).toEqual(["canvas-design"]);
  await user.click(screen.getByRole("button", { name: "All 3" }));

  await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "slop");
  expect(names()).toEqual(["antislop"]);
  await user.clear(screen.getByRole("searchbox", { name: "Search skills" }));
  await user.type(screen.getByRole("searchbox", { name: "Search skills" }), "zzz");
  expect(screen.getByText('No skill matches "zzz".')).toBeInTheDocument();
});

test("the detail dialog gives a copy command per folder and copies it", async () => {
  const user = userEvent.setup();
  backend({ scan_skills: () => skillScan() });
  render(SkillsPage);
  await user.click(await screen.findByRole("button", { name: "browseros-neo" }));

  const dialog = within(screen.getByRole("dialog", { name: "browseros-neo" }));
  expect(dialog.getByRole("heading", { name: "Add to Codex CLI" })).toBeInTheDocument();
  expect(dialog.getByRole("heading", { name: "Replace in Shared" })).toBeInTheDocument();
  expect(dialog.getByText(/Files that exist only in that copy are deleted/)).toBeInTheDocument();
  expect(dialog.getByRole("heading", { name: "OpenCode uses a link" })).toBeInTheDocument();
  expect(dialog.getByText(/Update it there/)).toBeInTheDocument();

  await user.click(dialog.getAllByRole("button", { name: "Copy command" })[0]);
  expect(await navigator.clipboard.readText()).toBe(
    `Copy-Item -LiteralPath '${H}\\.claude\\skills\\browseros-neo' -Destination '${H}\\.codex\\skills\\browseros-neo' -Recurse`,
  );

  await user.selectOptions(dialog.getByRole("combobox", { name: "Copy from" }), "agents");
  expect(dialog.getByRole("heading", { name: "Replace in Claude Code" })).toBeInTheDocument();
  expect(dialog.getAllByText(new RegExp(`-LiteralPath '${H.replace(/\\/g, "\\\\")}\\\\\\.agents`)).length).toBeGreaterThan(0);
});

test("loading, error and empty states say what happened", async () => {
  const user = userEvent.setup();
  backend({ scan_skills: () => new Promise(() => {}) });
  const first = render(SkillsPage);
  expect(screen.getByRole("status")).toHaveTextContent("Reading skill folders…");
  first.unmount();

  const calls = backend({ scan_skills: () => new Error("Access is denied.") });
  const second = render(SkillsPage);
  expect(await screen.findByRole("heading", { name: "Skill folders could not be read" })).toBeInTheDocument();
  expect(screen.getByText("Access is denied.")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Try again" }));
  expect(calls.calls("scan_skills")).toHaveLength(2);
  second.unmount();

  backend({ scan_skills: () => skillScan({ skills: [] }) });
  render(SkillsPage);
  expect(await screen.findByRole("heading", { name: "No skills in these folders yet" })).toBeInTheDocument();
  expect(screen.getByText("~/.codex/skills")).toBeInTheDocument();
});
