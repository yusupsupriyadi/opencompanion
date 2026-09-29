import { expect, test } from "vitest";
import type { TerminalInfo } from "./api";
import { MAX_PANES, PaneLayout, TERM } from "./panes.svelte";
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

test("the session's own terminal leads the first tab, and every shell gets a tab of its own", () => {
  const { layout } = setup("fresh", ["a", "b"]);
  expect(panes(layout)).toEqual([[TERM], ["a"], ["b"]]);
  expect(layout.tabOf("b")).toEqual({ panes: ["b"], dir: "row", sizes: [1] });
});

test("a split shell joins the end of the tab, and the tab's terminals share it equally", () => {
  const { layout, open } = setup("split", ["a"]);
  layout.split(TERM, "b");
  open(["a", "b"]);
  expect(panes(layout)).toEqual([[TERM, "b"], ["a"]]);
  expect(layout.tabOf("b")?.sizes).toEqual([0.5, 0.5]);

  layout.split("b", "c");
  open(["a", "b", "c"]);
  expect(layout.tabOf(TERM)?.panes).toEqual([TERM, "b", "c"]);
  expect(layout.tabOf(TERM)?.sizes).toEqual([1 / 3, 1 / 3, 1 / 3]);
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

test("a closed terminal leaves its tab where it was, led by the next one, and the others take its share", () => {
  const { layout, open } = setup("close", ["a", "b"]);
  layout.split("a", "c");
  open(["a", "b", "c"]);
  layout.resize("a", 0, 0.7, 0.1);
  expect(panes(layout)).toEqual([[TERM], ["a", "c"], ["b"]]);

  open(["b", "c"]);
  expect(panes(layout)).toEqual([[TERM], ["c"], ["b"]]);
  expect(layout.tabOf("c")?.sizes).toEqual([1]);
});

test("a border moves only the two terminals beside it, and neither gets less than the minimum", () => {
  const { layout, open } = setup("resize", []);
  layout.split(TERM, "a");
  open(["a"]);
  layout.split(TERM, "b");
  open(["a", "b"]);

  layout.resize(TERM, 1, 0.5, 0.1);
  const [x, y, z] = layout.tabOf(TERM)!.sizes;
  expect(x).toBeCloseTo(1 / 3);
  expect(y).toBeCloseTo(0.5);
  expect(z).toBeCloseTo(1 / 6);

  layout.resize(TERM, 0, 0, 0.1);
  expect(layout.tabOf(TERM)!.sizes[0]).toBeCloseTo(0.1);
  expect(layout.tabOf(TERM)!.sizes[1]).toBeCloseTo(0.5 + 1 / 3 - 0.1);
});

test("a tab switches between side by side and stacked", () => {
  const { layout, open } = setup("flip", []);
  layout.split(TERM, "a");
  open(["a"]);
  layout.flip("a");
  expect(layout.tabOf(TERM)?.dir).toBe("column");
  layout.flip(TERM);
  expect(layout.tabOf(TERM)?.dir).toBe("row");
});

test("the session's layout is there again when its screen opens again, and other sessions start plain", () => {
  const first = setup("again", ["a"]);
  first.layout.split("a", "b");
  first.open(["a", "b"]);
  first.layout.flip("a");

  const back = setup("again", ["a", "b"]);
  expect(panes(back.layout)).toEqual([[TERM], ["a", "b"]]);
  expect(back.layout.tabOf("a")?.dir).toBe("column");
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
  expect(layout.tabOf(TERM)?.sizes).toEqual([1]);

  layout.move("a", { kind: "tab", at: 4 });
  expect(panes(layout)).toEqual([[TERM], ["c"], ["b"], ["a"]]);
});

test("a terminal joins the end of another tab or lands beside a terminal, with an equal share there", () => {
  const { layout, open } = setup("move-in", ["a", "b"]);
  layout.move("a", { kind: "join", tab: TERM });
  expect(panes(layout)).toEqual([[TERM, "a"], ["b"]]);
  expect(layout.tabOf(TERM)?.dir).toBe("row");

  layout.resize(TERM, 0, 0.8, 0.1);
  layout.move("b", { kind: "beside", pane: TERM, after: false, dir: "column" });
  const tab = layout.tabOf(TERM)!;
  expect(tab.panes).toEqual(["b", TERM, "a"]);
  // A split tab keeps its direction; the others keep their proportions in what is left.
  expect(tab.dir).toBe("row");
  expect(tab.sizes[0]).toBeCloseTo(1 / 3);
  expect(tab.sizes[1]).toBeCloseTo(0.8 * (2 / 3));
  expect(tab.sizes[2]).toBeCloseTo(0.2 * (2 / 3));
});

test("beside the only terminal of a tab, the side picked sets how the tab is split", () => {
  const { layout } = setup("move-dir", ["a", "b"]);
  layout.move("b", { kind: "beside", pane: "a", after: true, dir: "column" });
  expect(layout.tabOf("a")).toEqual({ panes: ["a", "b"], dir: "column", sizes: [0.5, 0.5] });
});

test("a terminal moves along its own tab and keeps its share", () => {
  const { layout, open } = setup("move-along", []);
  layout.split(TERM, "a");
  open(["a"]);
  layout.resize(TERM, 0, 0.7, 0.1);
  layout.move(TERM, { kind: "beside", pane: "a", after: true, dir: "row" });
  const tab = layout.tabOf("a")!;
  expect(tab.panes).toEqual(["a", TERM]);
  expect(tab.sizes[0]).toBeCloseTo(0.3);
  expect(tab.sizes[1]).toBeCloseTo(0.7);
});

test("a full tab takes nothing from elsewhere, and a terminal never lands beside itself or joins its own tab", () => {
  const ids = ["a", "b", "c", "x"];
  const { layout, open } = setup("move-refused", ids);
  for (const id of ["a", "b", "c"]) layout.move(id, { kind: "join", tab: TERM });
  open(ids);
  expect(layout.tabOf(TERM)?.panes).toHaveLength(MAX_PANES);
  expect(layout.canMove("x", { kind: "join", tab: TERM })).toBe(false);
  expect(layout.canMove("x", { kind: "beside", pane: "a", after: true, dir: "row" })).toBe(false);
  expect(layout.canMove("a", { kind: "beside", pane: "b", after: true, dir: "row" })).toBe(true);
  expect(layout.canMove("a", { kind: "beside", pane: "a", after: true, dir: "row" })).toBe(false);
  expect(layout.canMove("a", { kind: "join", tab: TERM })).toBe(false);
  layout.move("x", { kind: "join", tab: TERM });
  expect(panes(layout).at(-1)).toEqual(["x"]);
});
