// A unified diff (`git diff` output for one file) as rows for the viewer.

export type DiffRow =
  | { kind: "hunk"; text: string }
  | { kind: "add" | "del" | "ctx"; old: number | null; new: number | null; text: string }
  /** A line about the file rather than its text: new file, rename, "No newline at end of file". */
  | { kind: "note"; text: string };

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/;

export function parseDiff(patch: string): DiffRow[] {
  const rows: DiffRow[] = [];
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  for (const line of patch.replace(/\r\n/g, "\n").split("\n")) {
    const hunk = HUNK.exec(line);
    if (hunk) {
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      inHunk = true;
      rows.push({ kind: "hunk", text: line });
      continue;
    }
    if (!inHunk) {
      // The header: `diff --git`, `index`, `---` and `+++` repeat what the tab already says.
      if (/^(new file|deleted file|rename from|rename to|similarity index|old mode|new mode)/.test(line)) rows.push({ kind: "note", text: line });
      continue;
    }
    if (line.startsWith("+")) rows.push({ kind: "add", old: null, new: newNo++, text: line.slice(1) });
    else if (line.startsWith("-")) rows.push({ kind: "del", old: oldNo++, new: null, text: line.slice(1) });
    else if (line.startsWith(" ")) rows.push({ kind: "ctx", old: oldNo++, new: newNo++, text: line.slice(1) });
    else if (line.startsWith("\\")) rows.push({ kind: "note", text: line.slice(2) });
    else if (line.startsWith("diff --git")) inHunk = false;
  }
  return rows;
}
