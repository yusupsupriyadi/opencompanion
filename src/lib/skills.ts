import type { SkillEntry, SkillProblem, SkillRoot, SkillRow, SkillScan } from "./api";

export type SkillFilter = "all" | "different" | "problems";
type Shell = SkillScan["shell"];

export const PROBLEM_LABEL: Record<SkillProblem, string> = {
  noSkillMd: "No SKILL.md",
  brokenLink: "Broken link",
  unreadable: "Unreadable",
  tooLarge: "Too large",
};

export type Cell =
  | { kind: "missing" }
  | { kind: "same"; label: "Same" | "Installed" }
  | { kind: "version"; variant: string; modifiedAt: number | null }
  | { kind: "problem"; label: string };

export type CopyStep =
  | { rootId: string; kind: "copy" | "replace"; command: string }
  | { rootId: string; kind: "link"; target: string | null }
  | { rootId: string; kind: "nested" };

export function entryIn(row: SkillRow, rootId: string): SkillEntry | undefined {
  return row.entries.find((e) => e.rootId === rootId);
}

const isDifferent = (row: SkillRow) => row.variants > 1;
const hasProblem = (row: SkillRow) => row.entries.some((e) => e.problem);

export function cellFor(row: SkillRow, rootId: string): Cell {
  const e = entryIn(row, rootId);
  if (!e) return { kind: "missing" };
  if (e.problem) return { kind: "problem", label: PROBLEM_LABEL[e.problem] };
  if (isDifferent(row) && e.variant) return { kind: "version", variant: e.variant, modifiedAt: e.modifiedAt };
  return { kind: "same", label: row.entries.filter((x) => x.hash).length > 1 ? "Same" : "Installed" };
}

export function skillCounts(rows: SkillRow[]) {
  return { all: rows.length, different: rows.filter(isDifferent).length, problems: rows.filter(hasProblem).length };
}

export function filterSkills(rows: SkillRow[], filter: SkillFilter, query: string): SkillRow[] {
  const q = query.trim().toLowerCase();
  return rows.filter(
    (r) =>
      (filter === "all" || (filter === "different" ? isDifferent(r) : hasProblem(r))) &&
      (!q || r.name.toLowerCase().includes(q) || (r.description ?? "").toLowerCase().includes(q)),
  );
}

/** Copies that can be copied from, newest version first, then in folder order. */
export function skillSources(row: SkillRow): SkillEntry[] {
  return row.entries
    .map((e, i) => ({ e, i }))
    .filter(({ e }) => e.hash && !e.problem)
    .sort((a, b) => (a.e.variant ?? "").localeCompare(b.e.variant ?? "") || a.i - b.i)
    .map(({ e }) => e);
}

/** One step per folder that lacks the source version. A folder that is a link, or holds one,
 *  gets no command: `Remove-Item -Recurse` in Windows PowerShell 5.1 follows junctions and can
 *  delete the files they point to. */
export function copySteps(shell: Shell, roots: SkillRoot[], row: SkillRow, source: SkillEntry): CopyStep[] {
  const steps: CopyStep[] = [];
  for (const root of roots) {
    if (root.id === source.rootId) continue;
    const e = entryIn(row, root.id);
    if (!e) {
      const dest = joinPath(shell, root.path, row.name);
      steps.push({ rootId: root.id, kind: "copy", command: copyCommand(shell, source.path, dest, root.exists ? null : root.path, false) });
    } else if (e.linkTarget || e.problem === "brokenLink") {
      steps.push({ rootId: root.id, kind: "link", target: e.linkTarget });
    } else if (e.problem === "unreadable" || e.problem === "tooLarge" || e.hash === source.hash) {
      continue;
    } else if (e.containsLinks) {
      steps.push({ rootId: root.id, kind: "nested" });
    } else {
      steps.push({ rootId: root.id, kind: "replace", command: copyCommand(shell, source.path, e.path, null, true) });
    }
  }
  return steps;
}

export function copyCommand(shell: Shell, src: string, dest: string, makeRoot: string | null, replace: boolean): string {
  if (shell === "powershell") {
    const q = (s: string) => `'${s.replace(/'/g, "''")}'`;
    const parts: string[] = [];
    // -ErrorAction Stop makes a failure end the line, so Copy-Item never merges into a half-removed folder.
    if (makeRoot) parts.push(`New-Item -ItemType Directory -Force -Path ${q(makeRoot)} -ErrorAction Stop | Out-Null`);
    if (replace) parts.push(`Remove-Item -LiteralPath ${q(dest)} -Recurse -Force -ErrorAction Stop`);
    parts.push(`Copy-Item -LiteralPath ${q(src)} -Destination ${q(dest)} -Recurse`);
    return parts.join("; ");
  }
  const q = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
  const parts: string[] = [];
  if (makeRoot) parts.push(`mkdir -p ${q(makeRoot)}`);
  if (replace) parts.push(`rm -rf ${q(dest)}`);
  // -L copies what links point to, the same content the scan hashed.
  parts.push(`cp -RL ${q(src)} ${q(dest)}`);
  return parts.join(" && ");
}

export function joinPath(shell: Shell, dir: string, name: string): string {
  return dir.replace(/[\\/]+$/, "") + (shell === "powershell" ? "\\" : "/") + name;
}
