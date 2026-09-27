// Keys and pastes xterm gets wrong for the AI CLIs and shells behind ConPTY.

/** Claude Code attaches these from a pasted path; Codex takes them too. */
const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/** The image in a paste without text, such as a screenshot. Text wins when there is both. */
export function pastedImage(data: Pick<DataTransfer, "getData" | "files"> | null): File | null {
  if (!data || data.getData("text/plain")) return null;
  return Array.from(data.files).find((f) => IMAGE_TYPES.includes(f.type)) ?? null;
}

/** Quoted when it holds a space, so a shell reads it as one argument; the CLIs strip the quotes. */
export function pathForPaste(path: string): string {
  return /\s/.test(path) ? `"${path}"` : path;
}

/** Shift+Enter as win32-input-mode key down and up, which ConPTY asks for with `ESC[?9001h`. */
const WIN32_SHIFT_ENTER = "\x1b[13;28;13;1;16;1_\x1b[13;28;13;0;16;1_";

/**
 * What Shift+Enter sends in place of xterm's plain Enter, or null to keep it. The AI CLIs read
 * Alt+Enter (ESC CR) as a new line, as Claude Code's own terminal setup binds it. A shell needs the
 * real key, which only win32-input-mode carries: PowerShell then adds a line.
 */
export function shiftEnter(kind: "session" | "terminal", win32Input: boolean): string | null {
  if (kind === "session") return "\x1b\r";
  return win32Input ? WIN32_SHIFT_ENTER : null;
}
