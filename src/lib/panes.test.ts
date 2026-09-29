import { expect, test } from "vitest";
import type { TerminalInfo } from "./api";
import { MAX_PANES, PaneLayout, TERM, borders, rects, type PaneNode } from "./panes.svelte";
import { SessionShells } from "./shells.svelte";

// The layout of each session is kept for the life of the window, so every test uses a session of its own.
function setup(session: string, ids: string[]) {
  const shells = new SessionShells(session);
  const open = (next: string[]) => {
    shells.list = next.map((id) => ({ id, sessionId: session }) as TerminalInfo);
  };
  open(ids);
  return { layout: new PaneLayout(session, () => shells.list.map((x) => x.id)), open };
}

const panes = (layout: PaneLayout) => layout.tabs.map((t) => t.panes);
/** A tab's splits written out: `row(a, column(b, c))`. */
const shape = (node: PaneNode): string => (node.kind === "leaf" ? node.pane : `${node.dir}(${node.children.map(shape).join(", ")})`);
const shapeOf = (layout: PaneLayout, pane: string) => shape(layout.tabOf(pane)!.root);
const sizesOf = (layout: PaneLayout, pane: string) => {
  const root = layout.tabOf(pane)!.root;
  return root.kind === "split" ? root.sizes : [1];
};

test("the session's own terminal leads the first tab, and every shell gets a tab of its own", () => {
  const { layout } = setup("fresh", ["a", "b"]);
  expect(panes(layout)).toEqual([[TERM], ["a"], ["b"]]);
  expect(shapeOf(layout, "b")).toBe("b");
});

test("a split shell joins the end of the tab, and the tab's terminals share it equally", () => {
  const { layout, open } = setup("split", ["a"]);
  layout.split(TERM, "b");
  open(["a", "b"]);
  expect(panes(layout)).toEqual([[TERM, "b"], ["a"]]);
  expect(shapeOf(layout, "b")).toBe("row(term, b)");
  expect(sizesOf(layout, "b")).toEqual([0.5, 0.5]);

  layout.split("b", "c");
  open(["a", "b", "c"]);
  expect(shapeOf(layout, TERM)).toBe("row(term, b, c)");
  sizesOf(layout, TERM).forEach((s) => expect(s).toBeCloseTo(1 / 3));
});

test("a full tab takes no more terminals", () => {
  const ids: string[] = [];
  const { layout, open } = setup("full", []);
  for (let i = 1; i <= MAX_PANES; i++) {
    ids.push(`s${i}`);
    layout.split(TERM, `s${i}`);
    open([...ids]);
  }
  expect(layout.tabOf(TERM)?.panes).toHaveLength(MAX_PANES);
  expect(panes(layout).at(-1)).toEqual([`s${MAX_PANES}`]);
});

test("a terminal lands on any side of any terminal, one split inside another", () => {
  const { layout } = setup("free", ["a", "b", "c"]);
  layout.move("a", { kind: "beside", pane: TERM, side: "right" });
  layout.move("b", { kind: "beside", pane: "a", side: "bottom" });
  expect(shapeOf(layout, TERM)).toBe("row(term, column(a, b))");
  layout.move("c", { kind: "beside", pane: TERM, side: "top" });
  expect(shapeOf(layout, TERM)).toBe("row(column(c, term), column(a, b))");
  expect(layout.tabOf(TERM)?.panes).toEqual(["c", TERM, "a", "b"]);

  // Each terminal's place follows: the left half is split top and bottom, and so is the right one.
  const at = rects(layout.tabOf(TERM)!.root);
  expect(at.get("c")).toEqual({ x: 0, y: 0, w: 0.5, h: 0.5 });
  expect(at.get(TERM)).toEqual({ x: 0, y: 0.5, w: 0.5, h: 0.5 });
  expect(at.get("b")).toEqual({ x: 0.5, y: 0.5, w: 0.5, h: 0.5 });
});

test("beside a terminal of a split running the same way, it takes half of that terminal's share", () => {
  const { layout } = setup("half", ["a", "b"]);
  layout.move("a", { kind: "join", tab: TERM });
  layout.move("b", { kind: "beside", pane: "a", side: "left" });
  expect(shapeOf(layout, TERM)).toBe("row(term, b, a)");
  expect(sizesOf(layout, TERM)).toEqual([0.5, 0.25, 0.25]);
});

test("a closed terminal leaves its tab where it was, and a split left with one part folds away", () => {
  const { layout, open } = setup("close", ["a", "b", "c"]);
  layout.move("b", { kind: "beside", pane: "a", side: "right" });
  layout.move("c", { kind: "beside", pane: "b", side: "bottom" });
  expect(panes(layout)).toEqual([[TERM], ["a", "b", "c"]]);
  expect(shapeOf(layout, "a")).toBe("row(a, column(b, c))");

  open(["a", "c"]);
  expect(shapeOf(layout, "a")).toBe("row(a, c)");
  open(["c"]);
  expect(panes(layout)).toEqual([[TERM], ["c"]]);
});

test("a border moves only the two parts beside it, and keeps room for every terminal they line up", () => {
  const { layout } = setup("resize", ["a", "b", "c"]);
  layout.move("a", { kind: "beside", pane: TERM, side: "right" });
  layout.move("b", { kind: "beside", pane: TERM, side: "bottom" });
  layout.move("c", { kind: "beside", pane: TERM, side: "right" });
  expect(shapeOf(layout, TERM)).toBe("row(column(row(term, c), b), a)");

  // The left part lines up two terminals side by side, so it keeps two units.
  layout.resize(TERM, [], 0, 0, 0.1);
  expect(sizesOf(layout, TERM).map((s) => Number(s.toFixed(6)))).toEqual([0.2, 0.8]);
  layout.resize(TERM, [], 0, 1, 0.1);
  expect(sizesOf(layout, TERM).map((s) => Number(s.toFixed(6)))).toEqual([0.9, 0.1]);

  // A border inside a split moves only that split's parts.
  layout.resize(TERM, [0, 0], 0, 0.7, 0.1);
  const inner = borders(layout.tabOf(TERM)!.root).find((b) => b.path.join() === "0,0")!;
  expect(inner.share).toBeCloseTo(0.7);
  expect(sizesOf(layout, TERM).map((s) => Number(s.toFixed(6)))).toEqual([0.9, 0.1]);
});

test("borders sit between the parts of each split, each after the terminals before it", () => {
  const { layout } = setup("borders", ["a", "b"]);
  layout.move("a", { kind: "beside", pane: TERM, side: "right" });
  layout.move("b", { kind: "beside", pane: "a", side: "bottom" });
  const [outer, inner] = borders(layout.tabOf(TERM)!.root);
  expect(outer).toMatchObject({ path: [], i: 0, dir: "row", at: 0.5, share: 0.5, before: [TERM], after: ["a", "b"] });
  expect(inner).toMatchObject({ path: [1], i: 0, dir: "column", at: 0.5, box: { x: 0.5, y: 0, w: 0.5, h: 1 }, before: ["a"], after: ["b"] });
});

test("a tab turns side by side into top to bottom all the way down, and back", () => {
  const { layout } = setup("flip", ["a", "b"]);
  layout.move("a", { kind: "beside", pane: TERM, side: "right" });
  layout.move("b", { kind: "beside", pane: "a", side: "bottom" });
  layout.flip("a");
  expect(shapeOf(layout, TERM)).toBe("column(term, row(a, b))");
  layout.flip(TERM);
  expect(shapeOf(layout, TERM)).toBe("row(term, column(a, b))");
});

test("two terminals of a tab swap places, and one docks along a whole edge of it", () => {
  const { layout } = setup("swap", ["a", "b"]);
  layout.move("a", { kind: "beside", pane: TERM, side: "right" });
  layout.move("b", { kind: "beside", pane: "a", side: "bottom" });
  layout.swap(TERM, "b");
  expect(shapeOf(layout, TERM)).toBe("row(b, column(a, term))");

  layout.dock(TERM, "top");
  expect(shapeOf(layout, TERM)).toBe("column(term, row(b, a))");
  expect(sizesOf(layout, TERM)[0]).toBeCloseTo(1 / 3);
  layout.dock(TERM, "left");
  expect(shapeOf(layout, TERM)).toBe("row(term, b, a)");
  sizesOf(layout, TERM).forEach((s) => expect(s).toBeCloseTo(1 / 3));
});

test("the session's layout is there again when its screen opens again, and other sessions start plain", () => {
  const first = setup("again", ["a", "b"]);
  first.layout.move("b", { kind: "beside", pane: "a", side: "bottom" });

  const back = setup("again", ["a", "b"]);
  expect(panes(back.layout)).toEqual([[TERM], ["a", "b"]]);
  expect(shapeOf(back.layout, "a")).toBe("column(a, b)");
  expect(panes(setup("other", ["x"]).layout)).toEqual([[TERM], ["x"]]);
});

test("a tab moves with all its terminals to before another tab, or last", () => {
  const { layout, open } = setup("move-tab", ["a", "b"]);
  layout.split("a", "c");
  open(["a", "b", "c"]);
  layout.moveTab("c", 0);
  expect(panes(layout)).toEqual([["a", "c"], [TERM], ["b"]]);
  layout.moveTab(TERM, 3);
  expect(panes(layout)).toEqual([["a", "c"], ["b"], [TERM]]);
});

test("a terminal moves to a tab of its own, and the tab it leaves goes away when empty", () => {
  const { layout, open } = setup("move-out", ["a", "b"]);
  layout.split(TERM, "c");
  open(["a", "b", "c"]);
  layout.move("c", { kind: "tab", at: 1 });
  expect(panes(layout)).toEqual([[TERM], ["c"], ["a"], ["b"]]);
  expect(shapeOf(layout, TERM)).toBe(TERM);

  layout.move("a", { kind: "tab", at: 4 });
  expect(panes(layout)).toEqual([[TERM], ["c"], ["b"], ["a"]]);
});

test("a full tab takes nothing from elsewhere, and a terminal never lands beside itself or joins its own tab", () => {
  const ids = ["a", "b", "c", "x"];
  const { layout, open } = setup("move-refused", ids);
  for (const id of ["a", "b", "c"]) layout.move(id, { kind: "join", tab: TERM });
  open(ids);
  expect(layout.tabOf(TERM)?.panes).toHaveLength(MAX_PANES);
  expect(layout.canMove("x", { kind: "join", tab: TERM })).toBe(false);
  expect(layout.canMove("x", { kind: "beside", pane: "a", side: "right" })).toBe(false);
  expect(layout.canMove("a", { kind: "beside", pane: "b", side: "top" })).toBe(true);
  expect(layout.canMove("a", { kind: "beside", pane: "a", side: "right" })).toBe(false);
  expect(layout.canMove("a", { kind: "join", tab: TERM })).toBe(false);
  layout.move("x", { kind: "join", tab: TERM });
  expect(panes(layout).at(-1)).toEqual(["x"]);
});

test("pinned tabs come first in the order they were pinned, and nothing moves across that line", () => {
  const { layout } = setup("pin", ["a", "b", "c"]);
  layout.pin("c", true);
  layout.pin("a", true);
  expect(panes(layout)).toEqual([["c"], ["a"], [TERM], ["b"]]);
  expect(layout.tabOf("a")?.pinned).toBe(true);

  layout.moveTab("b", 0);
  expect(panes(layout)).toEqual([["c"], ["a"], ["b"], [TERM]]);
  layout.move("b", { kind: "beside", pane: "a", side: "right" });
  expect(panes(layout)).toEqual([["c"], ["a", "b"], [TERM]]);
  expect(layout.tabOf("b")?.pinned).toBe(true);

  layout.pin("c", false);
  expect(panes(layout)).toEqual([["a", "b"], ["c"], [TERM]]);
});

test("a tab keeps its name through moves and when its first terminal closes, and a new tab goes where it is put", () => {
  const { layout, open } = setup("name", ["a", "b"]);
  layout.move("b", { kind: "join", tab: "a" });
  layout.rename("a", "  dev  ");
  expect(layout.tabOf("b")?.title).toBe("dev");
  layout.moveTab("a", 0);
  open(["b"]);
  expect(layout.tabOf("b")?.title).toBe("dev");
  layout.rename("b", "   ");
  expect(layout.tabOf("b")?.title).toBeUndefined();

  layout.insertTab("c", 1);
  open(["b", "c"]);
  expect(panes(layout)).toEqual([["b"], ["c"], [TERM]]);
});
