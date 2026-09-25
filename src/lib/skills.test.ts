import { expect, test } from "vitest";
import { skillScan } from "../test/skill-scan";
import { cellFor, copyCommand, copySteps, filterSkills, skillCounts, skillSources } from "./skills";

const scan = skillScan();
const [antislop, neo, canvas] = scan.skills;
const H = String.raw`C:\Users\me`;

test("cells say whether a folder has the skill, which version, or what is wrong", () => {
  expect(cellFor(antislop, "claude")).toEqual({ kind: "same", label: "Same" });
  expect(cellFor(antislop, "agents")).toEqual({ kind: "missing" });
  expect(cellFor(neo, "claude")).toMatchObject({ kind: "version", variant: "A" });
  expect(cellFor(neo, "agents")).toMatchObject({ kind: "version", variant: "B" });
  expect(cellFor(canvas, "agents")).toEqual({ kind: "problem", label: "No SKILL.md" });
  const single = { ...antislop, entries: antislop.entries.slice(0, 1) };
  expect(cellFor(single, "claude")).toEqual({ kind: "same", label: "Installed" });
});

test("counts and filters follow versions, problems and the search text", () => {
  expect(skillCounts(scan.skills)).toEqual({ all: 3, different: 1, problems: 1 });
  expect(filterSkills(scan.skills, "different", "").map((r) => r.name)).toEqual(["browseros-neo"]);
  expect(filterSkills(scan.skills, "problems", "").map((r) => r.name)).toEqual(["canvas-design"]);
  expect(filterSkills(scan.skills, "all", " slop ").map((r) => r.name)).toEqual(["antislop"]);
  expect(filterSkills(scan.skills, "all", "BROWSER").map((r) => r.name)).toEqual(["browseros-neo"]);
  expect(filterSkills(scan.skills, "different", "slop")).toEqual([]);
});

test("sources are readable copies, newest version first", () => {
  expect(skillSources(neo).map((e) => e.rootId)).toEqual(["claude", "agents", "opencode"]);
  expect(skillSources(canvas)).toEqual([]);
});

test("steps copy into folders that lack the skill and replace other versions, but never through a link", () => {
  const steps = copySteps(scan.shell, scan.roots, neo, neo.entries[0]);
  expect(steps.map((s) => [s.rootId, s.kind])).toEqual([
    ["codex", "copy"],
    ["agents", "replace"],
    ["opencode", "link"],
    ["gemini", "copy"],
  ]);
  const src = `'${H}\\.claude\\skills\\browseros-neo'`;
  expect(steps[0]).toMatchObject({
    command: `Copy-Item -LiteralPath ${src} -Destination '${H}\\.codex\\skills\\browseros-neo' -Recurse`,
  });
  expect(steps[1]).toMatchObject({
    command:
      `Remove-Item -LiteralPath '${H}\\.agents\\skills\\browseros-neo' -Recurse -Force -ErrorAction Stop; ` +
      `Copy-Item -LiteralPath ${src} -Destination '${H}\\.agents\\skills\\browseros-neo' -Recurse`,
  });
  expect(steps[2]).toEqual({ rootId: "opencode", kind: "link", target: `${H}\\.agents\\skills\\browseros-neo` });
  expect(steps[3]).toMatchObject({
    command:
      `New-Item -ItemType Directory -Force -Path '${H}\\.gemini\\skills' -ErrorAction Stop | Out-Null; ` +
      `Copy-Item -LiteralPath ${src} -Destination '${H}\\.gemini\\skills\\browseros-neo' -Recurse`,
  });
});

test("a folder with a link inside gets no replace command", () => {
  const agents = { ...neo.entries[1], containsLinks: true };
  const row = { ...neo, entries: [neo.entries[0], agents, neo.entries[2]] };
  const step = copySteps(scan.shell, scan.roots, row, neo.entries[0]).find((s) => s.rootId === "agents");
  expect(step).toEqual({ rootId: "agents", kind: "nested" });
});

test("folders that already hold the source version get no step", () => {
  const steps = copySteps(scan.shell, scan.roots, antislop, antislop.entries[0]);
  expect(steps.map((s) => s.rootId)).toEqual(["agents", "gemini"]);
});

test("quotes survive in both shells", () => {
  expect(copyCommand("powershell", String.raw`C:\a\it's`, String.raw`C:\b\it's`, null, false)).toBe(
    String.raw`Copy-Item -LiteralPath 'C:\a\it''s' -Destination 'C:\b\it''s' -Recurse`,
  );
  expect(copyCommand("sh", "/h/.claude/skills/it's", "/h/.codex/skills/it's", "/h/.codex/skills", true)).toBe(
    `mkdir -p '/h/.codex/skills' && rm -rf '/h/.codex/skills/it'\\''s' && cp -RL '/h/.claude/skills/it'\\''s' '/h/.codex/skills/it'\\''s'`,
  );
});
