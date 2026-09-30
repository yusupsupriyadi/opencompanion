// OSC 52: how Claude Code over SSH, tmux and Neovim copy through the terminal, which xterm leaves to us. As in Orca
// (src/renderer/src/components/terminal-pane/osc52-clipboard.ts), only writes count, never reading (`?`) or clearing.

// Over a screenful of text, well under xterm's own 10 MB limit on an OSC sequence.
const MAX_BASE64 = 1024 * 1024;

/** The text an OSC 52 sequence copies, given what follows `ESC ] 52 ;`, or null when it copies nothing. */
export function osc52Text(data: string): string | null {
  const semi = data.indexOf(";");
  if (semi < 0) return null;
  // Every selection lands in the clipboard. tmux sends none at all.
  if (!/^[cpqs0-7]*$/.test(data.slice(0, semi))) return null;
  const payload = data.slice(semi + 1).replace(/\s+/g, "");
  if (!payload || payload.length > MAX_BASE64 || !/^[A-Za-z0-9+/]+={0,2}$/.test(payload)) return null;
  try {
    const bytes = Uint8Array.from(atob(payload), (c) => c.charCodeAt(0));
    return new TextDecoder().decode(bytes) || null;
  } catch {
    return null;
  }
}
