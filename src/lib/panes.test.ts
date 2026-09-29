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
