// How a session's terminals share the tabs above its panel: each tab holds one terminal, or several split side by side
// or stacked top to bottom. The session's own terminal ("term") always leads the first tab.

export const TERM = "term";
/** More than this and each terminal gets too narrow to read at the smallest window. */
export const MAX_PANES = 4;

export type PaneDir = "row" | "column";
/** A tab's terminals in order; the first one names the tab. `sizes` are their shares of the tab, adding up to 1. */
export type PaneTab = { panes: string[]; dir: PaneDir; sizes: number[] };
/** Where a moved terminal lands: in a tab of its own before tab `at`, at the end of the tab holding `tab`, or beside `pane`. */
export type PaneDrop =
  | { kind: "tab"; at: number }
  | { kind: "join"; tab: string }
  | { kind: "beside"; pane: string; after: boolean; dir: PaneDir };

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

  /** Moves the tab holding `pane`, with all its terminals, to before tab `at`; the tab count puts it last. */
  moveTab(pane: string, at: number) {
    const tabs = [...this.tabs];
    const from = tabs.findIndex((t) => t.panes.includes(pane));
    if (from < 0) return;
    const [tab] = tabs.splice(from, 1);
    tabs.splice(from < at ? at - 1 : at, 0, tab);
    this.#save(tabs);
  }

  /** A full tab takes no terminal from another tab, and a terminal cannot land beside itself or join its own tab. */
  canMove(pane: string, to: PaneDrop): boolean {
    const from = this.tabOf(pane);
    if (!from) return false;
    if (to.kind === "tab") return true;
    const into = this.tabOf(to.kind === "join" ? to.tab : to.pane);
    if (!into || (to.kind === "beside" && to.pane === pane)) return false;
    if (into.panes[0] === from.panes[0]) return to.kind === "beside";
    return into.panes.length < MAX_PANES;
  }

  /**
   * Takes `pane` out of its tab and puts it at `to`; a tab left empty goes away. In its own tab it keeps its share;
   * in another it gets an equal one. Beside the only terminal of a tab, `to.dir` sets how that tab is split.
   */
  move(pane: string, to: PaneDrop) {
    if (!this.canMove(pane, to)) return;
    const tabs = this.tabs;
    const src = tabs.findIndex((t) => t.panes.includes(pane));
    const own = tabs[src];
    const at = own.panes.indexOf(pane);
    // The emptied tab stays in place until the end, so `to.at` still counts the tabs as they were.
    const next = tabs.map((t, i) => (i === src ? { ...t, panes: t.panes.filter((p) => p !== pane), sizes: t.sizes.filter((_, j) => j !== at) } : t));
    if (to.kind === "tab") next.splice(to.at, 0, { panes: [pane], dir: "row", sizes: [1] });
    else {
      const ti = next.findIndex((t) => t.panes.includes(to.kind === "join" ? to.tab : to.pane));
      const t = next[ti];
      const index = to.kind === "join" ? t.panes.length : t.panes.indexOf(to.pane) + (to.after ? 1 : 0);
      const n = t.panes.length + 1;
      const size = ti === src ? own.sizes[at] : 1 / n;
      const rest = ti === src ? 1 : (1 - size) / t.sizes.reduce((a, b) => a + b, 0);
      const sizes = t.sizes.map((x) => x * rest);
      next[ti] = {
        panes: [...t.panes.slice(0, index), pane, ...t.panes.slice(index)],
        dir: to.kind === "beside" && t.panes.length === 1 ? to.dir : t.dir,
        sizes: [...sizes.slice(0, index), size, ...sizes.slice(index)],
      };
    }
    this.#save(next.filter((t) => t.panes.length > 0));
  }

  #save(next: PaneTab[]) {
    this.#tabs = next;
    kept.set(this.sessionId, next);
  }
}
