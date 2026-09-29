// How a session's terminals share the tabs above its panel: each tab holds one terminal, or several split side by side
// or stacked top to bottom. The session's own terminal ("term") always leads the first tab.

export const TERM = "term";
/** More than this and each terminal gets too narrow to read at the smallest window. */
export const MAX_PANES = 4;

export type PaneDir = "row" | "column";
/** A tab's terminals in order; the first one names the tab. `sizes` are their shares of the tab, adding up to 1. */
export type PaneTab = { panes: string[]; dir: PaneDir; sizes: number[] };

// Per session, for as long as the window is open: shells live that long too, and going to another screen and back
// finds them split as they were.
const kept = new Map<string, PaneTab[]>();

export class PaneLayout {
  #tabs = $state.raw<PaneTab[]>([]);

  /** The tabs in order, with closed shells left out and shells not placed yet in a tab of their own. */
  tabs = $derived.by(() => {
    const ids = this.ids();
    const live = new Set(ids);
    const placed = new Set<string>();
    const out: PaneTab[] = [];
    for (const tab of this.#tabs) {
      const at = tab.panes.flatMap((p, i) => ((p === TERM || live.has(p)) && !placed.has(p) ? [i] : []));
      if (!at.length) continue;
      at.forEach((i) => placed.add(tab.panes[i]));
      const sizes = at.map((i) => (tab.sizes[i] > 0 ? tab.sizes[i] : 1 / at.length));
      const sum = sizes.reduce((a, b) => a + b, 0);
      out.push({ panes: at.map((i) => tab.panes[i]), dir: tab.dir, sizes: sizes.map((x) => x / sum) });
    }
    if (!placed.has(TERM)) out.unshift({ panes: [TERM], dir: "row", sizes: [1] });
    for (const id of ids) if (!placed.has(id)) out.push({ panes: [id], dir: "row", sizes: [1] });
    return out;
  });

  /** `ids` gives the session's open shells in the order they were opened. */
  constructor(
    readonly sessionId: string,
    readonly ids: () => string[],
  ) {
    this.#tabs = kept.get(sessionId) ?? [];
  }

  tabOf(pane: string): PaneTab | undefined {
    return this.tabs.find((t) => t.panes.includes(pane));
  }

  /** Puts a new shell after the last terminal of `pane`'s tab; every terminal there then gets the same share. */
  split(pane: string, id: string) {
    this.#save(
      this.tabs.map((t) => {
        if (!t.panes.includes(pane) || t.panes.length >= MAX_PANES) return t;
        const n = t.panes.length + 1;
        return { ...t, panes: [...t.panes, id], sizes: Array(n).fill(1 / n) };
      }),
    );
  }

  /** Switches a tab between side by side and stacked. */
  flip(pane: string) {
    this.#save(this.tabs.map((t) => (t.panes.includes(pane) ? { ...t, dir: t.dir === "row" ? "column" : "row" } : t)));
  }

  /**
   * Moves the border after the tab's `i`th terminal so that terminal gets `size` of the tab. Only it and the next one
   * change, and neither gets less than `min`.
   */
  resize(pane: string, i: number, size: number, min: number) {
    this.#save(
      this.tabs.map((t) => {
        if (!t.panes.includes(pane) || i < 0 || i + 1 >= t.panes.length) return t;
        const pair = t.sizes[i] + t.sizes[i + 1];
        const low = Math.min(min, pair / 2);
        const a = Math.min(Math.max(size, low), pair - low);
        const sizes = [...t.sizes];
        sizes[i] = a;
        sizes[i + 1] = pair - a;
        return { ...t, sizes };
      }),
    );
  }

  #save(next: PaneTab[]) {
    this.#tabs = next;
    kept.set(this.sessionId, next);
  }
}
