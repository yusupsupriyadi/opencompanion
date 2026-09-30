// Suggesting commands in a shell tab. The shell's script marks where typing starts (OSC 133;B), so what was typed is
// read back from xterm's buffer between that mark and the cursor, whatever keys edited it.

/** The part of xterm's buffer the typed text is read from. */
export interface BufferRows {
  getLine(y: number): { isWrapped: boolean; translateToString(trimRight?: boolean, start?: number, end?: number): string } | undefined;
}

/** A position in the buffer: `y` counts rows from the top of the scrollback, `x` cells. */
export interface Cell {
  y: number;
  x: number;
}

/** Shells whose tabs start with the script (`shell_integration::kind` in Rust), by `TerminalInfo.shell`. */
export const SUGGESTING_SHELLS = new Set(["pwsh", "powershell", "gitbash", "bash", "zsh"]);

/**
 * The text typed at the prompt marked at `mark`, or null when it cannot be read as one line ending at the cursor: the
 * cursor is before the mark or inside the line, or a row between them is a row of its own rather than a wrap (a
 * continuation prompt, or output).
 */
export function typedText(buffer: BufferRows, mark: Cell, cursor: Cell): string | null {
  if (cursor.y < mark.y || (cursor.y === mark.y && cursor.x < mark.x)) return null;
  let text = "";
  for (let y = mark.y; y <= cursor.y; y++) {
    const line = buffer.getLine(y);
    if (!line || (y > mark.y && !line.isWrapped)) return null;
    text += line.translateToString(false, y === mark.y ? mark.x : 0, y === cursor.y ? cursor.x : undefined);
  }
  const last = buffer.getLine(cursor.y);
  if (last?.translateToString(true, cursor.x) || buffer.getLine(cursor.y + 1)?.isWrapped) return null;
  return text;
}

/** The first command in `lists`, in order, that starts with what was typed and goes on past it. */
export function suggestion(typed: string, lists: string[][]): string | null {
  if (!typed.trim()) return null;
  for (const list of lists) {
    const hit = list.find((c) => c.length > typed.length && c.startsWith(typed));
    if (hit) return hit;
  }
  return null;
}
