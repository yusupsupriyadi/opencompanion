// How a session's terminals share the tabs above its panel: each tab holds one terminal, or several split freely, side
// by side and top to bottom, one split inside another. The session's own terminal ("term") starts in the first tab.

export const TERM = "term";
/** More than this and each terminal gets too small to read at the smallest window. */
export const MAX_PANES = 4;

export type PaneDir = "row" | "column";
export type PaneSide = "left" | "right" | "top" | "bottom";
/**
 * A terminal, or a split of two or more parts running one way, each with its share of the split (adding up to 1).
 * A part never runs the same way as the split holding it: such a part is folded into it.
 */
export type PaneNode = { kind: "leaf"; pane: string } | { kind: "split"; dir: PaneDir; children: PaneNode[]; sizes: number[] };
/** `panes` are the tab's terminals in reading order; the first one names the tab. */
export type PaneTab = { root: PaneNode; panes: string[] };
/** Where a moved terminal lands: in a tab of its own before tab `at`, at the end of the tab holding `tab`, or beside `pane`. */
export type PaneDrop = { kind: "tab"; at: number } | { kind: "join"; tab: string } | { kind: "beside"; pane: string; side: PaneSide };
/** A terminal's place in its tab, as fractions of the tab's width and height. */
export type PaneRect = { x: number; y: number; w: number; h: number };
/**
 * The border between parts `i` and `i + 1` of the split at `path` (child indexes from the tab's root). `at` is where it
 * sits along the split's direction, and `box` is the split's own place, both as fractions of the tab.
 */
export type PaneBorder = { path: number[]; i: number; dir: PaneDir; at: number; box: PaneRect; share: number; before: string[]; after: string[] };

const leaf = (pane: string): PaneNode => ({ kind: "leaf", pane });

export function leaves(node: PaneNode): string[] {
  return node.kind === "leaf" ? [node.pane] : node.children.flatMap(leaves);
}

/** How many terminals a node lines up along `dir`, so a border keeps room for each of them. */
function span(node: PaneNode, dir: PaneDir): number {
  if (node.kind === "leaf") return 1;
  const each = node.children.map((c) => span(c, dir));
  return node.dir === dir ? each.reduce((a, b) => a + b, 0) : Math.max(...each);
}

/** Keeps the terminals `keep` accepts, folds splits left with one part or running their holder's way, and rescales. */
function prune(node: PaneNode, keep: (pane: string) => boolean): PaneNode | null {
  if (node.kind === "leaf") return keep(node.pane) ? node : null;
  const children: PaneNode[] = [];
  const sizes: number[] = [];
  node.children.forEach((c, i) => {
    const kept = prune(c, keep);
    const share = node.sizes[i] > 0 ? node.sizes[i] : 1 / node.children.length;
    if (!kept) return;
    if (kept.kind === "split" && kept.dir === node.dir) {
      children.push(...kept.children);
      sizes.push(...kept.sizes.map((s) => s * share));
    } else {
      children.push(kept);
      sizes.push(share);
    }
  });
  if (!children.length) return null;
  if (children.length === 1) return children[0];
  const sum = sizes.reduce((a, b) => a + b, 0);
  return { kind: "split", dir: node.dir, children, sizes: sizes.map((s) => s / sum) };
}

const tidy = (node: PaneNode) => prune(node, () => true) ?? node;

/** Adds `pane` at the end of the tab, running the tab's own way (side by side for a single terminal). */
function append(root: PaneNode, pane: string): PaneNode {
  if (root.kind === "leaf") return { kind: "split", dir: "row", children: [root, leaf(pane)], sizes: [0.5, 0.5] };
  const n = root.children.length + 1;
  return { ...root, children: [...root.children, leaf(pane)], sizes: [...root.sizes.map((s) => (s * (n - 1)) / n), 1 / n] };
}

/** Puts `pane` on one side of `target`, which gives it half of its place. */
function beside(root: PaneNode, target: string, pane: string, side: PaneSide): PaneNode {
  const dir: PaneDir = side === "left" || side === "right" ? "row" : "column";
  const first = side === "left" || side === "top";
  const go = (node: PaneNode): PaneNode => {
    if (node.kind === "leaf") {
      if (node.pane !== target) return node;
      return { kind: "split", dir, children: first ? [leaf(pane), node] : [node, leaf(pane)], sizes: [0.5, 0.5] };
    }
    const i = node.children.findIndex((c) => c.kind === "leaf" && c.pane === target);
    if (i >= 0 && node.dir === dir) {
      const at = first ? i : i + 1;
      const half = node.sizes[i] / 2;
      const sizes = node.sizes.map((s, j) => (j === i ? half : s));
      return { ...node, children: [...node.children.slice(0, at), leaf(pane), ...node.children.slice(at)], sizes: [...sizes.slice(0, at), half, ...sizes.slice(at)] };
    }
    return { ...node, children: node.children.map(go) };
  };
  return tidy(go(root));
}

/** Turns side by side into top to bottom and back, all the way down. */
function turn(node: PaneNode): PaneNode {
  return node.kind === "leaf" ? node : { ...node, dir: node.dir === "row" ? "column" : "row", children: node.children.map(turn) };
}

function swap(node: PaneNode, a: string, b: string): PaneNode {
  if (node.kind === "leaf") return node.pane === a ? leaf(b) : node.pane === b ? leaf(a) : node;
  return { ...node, children: node.children.map((c) => swap(c, a, b)) };
}

/** Every terminal's place in the tab. */
export function rects(node: PaneNode, box: PaneRect = { x: 0, y: 0, w: 1, h: 1 }, out = new Map<string, PaneRect>()) {
  if (node.kind === "leaf") return out.set(node.pane, box);
  let at = 0;
  node.children.forEach((c, i) => {
    const s = node.sizes[i];
    rects(c, node.dir === "row" ? { ...box, x: box.x + at * box.w, w: s * box.w } : { ...box, y: box.y + at * box.h, h: s * box.h }, out);
    at += s;
  });
  return out;
}

/** Every border of the tab in reading order: each one right after the terminals before it. */
export function borders(node: PaneNode, box: PaneRect = { x: 0, y: 0, w: 1, h: 1 }, path: number[] = []): PaneBorder[] {
  if (node.kind === "leaf") return [];
  const row = node.dir === "row";
  const out: PaneBorder[] = [];
  let at = 0;
  node.children.forEach((c, i) => {
    const s = node.sizes[i];
    out.push(...borders(c, row ? { ...box, x: box.x + at * box.w, w: s * box.w } : { ...box, y: box.y + at * box.h, h: s * box.h }, [...path, i]));
    at += s;
    if (i + 1 < node.children.length) {
      out.push({
        path,
        i,
        dir: node.dir,
        at: row ? box.x + at * box.w : box.y + at * box.h,
        box,
        share: s / (s + node.sizes[i + 1]),
        before: leaves(c),
        after: leaves(node.children[i + 1]),
      });
    }
  });
  return out;
}

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
      const root = prune(tab.root, (p) => (p === TERM || live.has(p)) && !placed.has(p));
      if (!root) continue;
      const panes = leaves(root);
      panes.forEach((p) => placed.add(p));
      out.push({ root, panes });
    }
    if (!placed.has(TERM)) out.unshift({ root: leaf(TERM), panes: [TERM] });
    for (const id of ids) if (!placed.has(id)) out.push({ root: leaf(id), panes: [id] });
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

  /** Puts a new shell at the end of `pane`'s tab, with an equal share. */
  split(pane: string, id: string) {
    this.#edit(pane, (root, tab) => (tab.panes.length >= MAX_PANES ? root : append(root, id)));
  }

  /** Turns a tab's splits between side by side and top to bottom. */
  flip(pane: string) {
    this.#edit(pane, turn);
  }

  /** Puts `pane` along one whole edge of its tab with an even share, and the rest of the tab beside it. */
  dock(pane: string, side: PaneSide) {
    this.#edit(pane, (root, tab) => {
      const rest = prune(root, (p) => p !== pane);
      if (!rest) return root;
      const share = 1 / tab.panes.length;
      const first = side === "left" || side === "top";
      return {
        kind: "split",
        dir: side === "left" || side === "right" ? "row" : "column",
        children: first ? [leaf(pane), rest] : [rest, leaf(pane)],
        sizes: first ? [share, 1 - share] : [1 - share, share],
      };
    });
  }

  /** Swaps two terminals of one tab. */
  swap(a: string, b: string) {
    this.#edit(a, (root, tab) => (tab.panes.includes(b) ? swap(root, a, b) : root));
  }

  /**
   * Moves border `i` of the split at `path` so part `i` gets `size` of that split. Only it and the next part change,
   * and each keeps `unit` (a share of the split) for every terminal it lines up.
   */
  resize(pane: string, path: number[], i: number, size: number, unit: number) {
    const go = (node: PaneNode, depth: number): PaneNode => {
      if (node.kind === "leaf") return node;
      if (depth < path.length) return { ...node, children: node.children.map((c, j) => (j === path[depth] ? go(c, depth + 1) : c)) };
      if (i < 0 || i + 1 >= node.children.length) return node;
      const pair = node.sizes[i] + node.sizes[i + 1];
      let low = span(node.children[i], node.dir) * unit;
      let high = span(node.children[i + 1], node.dir) * unit;
      if (low + high > pair) [low, high] = [(pair * low) / (low + high), (pair * high) / (low + high)];
      const a = Math.min(Math.max(size, low), pair - high);
      return { ...node, sizes: node.sizes.map((s, j) => (j === i ? a : j === i + 1 ? pair - a : s)) };
    };
    this.#edit(pane, (root) => go(root, 0));
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

  /** Takes `pane` out of its tab and puts it at `to`; a tab left empty goes away. */
  move(pane: string, to: PaneDrop) {
    if (!this.canMove(pane, to)) return;
    // The emptied tab stays in place until the end, so `to.at` still counts the tabs as they were.
    const next: (PaneNode | null)[] = this.tabs.map((t) => (t.panes.includes(pane) ? prune(t.root, (p) => p !== pane) : t.root));
    if (to.kind === "tab") next.splice(to.at, 0, leaf(pane));
    else {
      const target = to.kind === "join" ? to.tab : to.pane;
      const ti = next.findIndex((r) => r !== null && leaves(r).includes(target));
      const root = next[ti]!;
      next[ti] = to.kind === "join" ? append(root, pane) : beside(root, target, pane, to.side);
    }
    this.#save(next.filter((r): r is PaneNode => r !== null).map((root) => ({ root, panes: leaves(root) })));
  }

  #edit(pane: string, change: (root: PaneNode, tab: PaneTab) => PaneNode) {
    this.#save(
      this.tabs.map((t) => {
        if (!t.panes.includes(pane)) return t;
        const root = tidy(change(t.root, t));
        return { root, panes: leaves(root) };
      }),
    );
  }

  #save(next: PaneTab[]) {
    this.#tabs = next;
    kept.set(this.sessionId, next);
  }
}
