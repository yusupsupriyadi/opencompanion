// The phone's terminal view. The desktop sends the screen as plain text: its vt100 parser has
// already applied the escape codes (companion::render_screen), so no ANSI reaches the phone. What
// does reach it is the box drawing TUIs frame their input with (╭───╮ │ > │ ╰───╯) and full-width
// rules, drawn for a terminal about 100 columns wide. Wrapped to a phone's 45 or so columns they
// turn into rows of dashes, so boxes and rules are drawn by the page instead.

/** A run of plain lines, a full-width rule, or a framed box with an optional title. */
export type TailBlock =
  | { kind: "text"; text: string }
  | { kind: "rule" }
  | { kind: "box"; title: string; lines: (string | null)[] };

const BOX = "─-╿";
/** Only box drawing (plus a few ASCII stand-ins) and spaces: a rule, or a box's top or bottom edge. */
const EDGE = new RegExp(`^[${BOX}\\s=_-]+$`);
const TOP = /^[╭┌╔┏]/;
const BOTTOM = /^[╰└╚┗]/;
const SIDE = /^[│┃║]/;
const SIDE_END = /[│┃║]$/;
const SIDE_RULE = /^[├┝┠╟┣]/;

function isEdge(line: string): boolean {
  return line.length >= 3 && EDGE.test(line) && [...line].filter((c) => c !== " ").length >= 3;
}

/** A top edge with its title: `╭─── Claude Code ───╮` or a plain `╭──────╮`. */
function topTitle(line: string): string | null {
  if (!TOP.test(line) || !/[╮┐╗┓]$/.test(line)) return null;
  return line.replace(new RegExp(`[${BOX}]`, "g"), " ").replace(/\s+/g, " ").trim();
}

/** Splits a screen into blocks. Blank runs collapse to one blank line, and the edges of the screen lose theirs. */
export function parseTail(tail: string): TailBlock[] {
  const blocks: TailBlock[] = [];
  let text: string[] = [];
  let box: { kind: "box"; title: string; lines: (string | null)[] } | null = null;

  const flushText = () => {
    while (text.length && !text[text.length - 1].trim()) text.pop();
    if (text.length) blocks.push({ kind: "text", text: text.join("\n") });
    text = [];
  };
  const closeBox = () => {
    if (box) blocks.push(box);
    box = null;
  };

  for (const raw of tail.replace(/\r/g, "").split("\n")) {
    const line = raw.trimEnd();
    const lead = line.trimStart();

    if (box) {
      if (BOTTOM.test(lead) && isEdge(lead)) {
        closeBox();
        continue;
      }
      if (SIDE_RULE.test(lead) && isEdge(lead)) {
        box.lines.push(null);
        continue;
      }
      if (SIDE.test(lead) && SIDE_END.test(lead) && lead.length > 1) {
        const inner = lead.slice(1, -1).replace(/^ /, "").trimEnd();
        if (inner || box.lines[box.lines.length - 1] !== "") box.lines.push(inner);
        continue;
      }
      // A box the screen cut off: keep what it had, and read this line on its own.
      closeBox();
    }

    const title = topTitle(lead);
    if (title !== null) {
      flushText();
      box = { kind: "box", title, lines: [] };
      continue;
    }
    if (isEdge(lead)) {
      flushText();
      blocks.push({ kind: "rule" });
      continue;
    }
    if (!line.trim()) {
      if (text.length && text[text.length - 1].trim()) text.push("");
      continue;
    }
    text.push(line);
  }
  closeBox();
  flushText();

  // Blank lines left at the start or end of a box say nothing on a phone.
  for (const b of blocks) {
    if (b.kind !== "box") continue;
    while (b.lines.length && b.lines[0] === "") b.lines.shift();
    while (b.lines.length && b.lines[b.lines.length - 1] === "") b.lines.pop();
  }
  return blocks;
}
