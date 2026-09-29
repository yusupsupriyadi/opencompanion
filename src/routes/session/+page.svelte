<script lang="ts">
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import ArrowDown from "phosphor-svelte/lib/ArrowDown";
  import ArrowLeft from "phosphor-svelte/lib/ArrowLeft";
  import ArrowRight from "phosphor-svelte/lib/ArrowRight";
  import ArrowsLeftRight from "phosphor-svelte/lib/ArrowsLeftRight";
  import ArrowUp from "phosphor-svelte/lib/ArrowUp";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import Files from "phosphor-svelte/lib/Files";
  import GitBranch from "phosphor-svelte/lib/GitBranch";
  import GitDiff from "phosphor-svelte/lib/GitDiff";
  import Info from "phosphor-svelte/lib/Info";
  import Play from "phosphor-svelte/lib/Play";
  import Plus from "phosphor-svelte/lib/Plus";
  import Rows from "phosphor-svelte/lib/Rows";
  import SidebarSimple from "phosphor-svelte/lib/SidebarSimple";
  import SquareSplitHorizontal from "phosphor-svelte/lib/SquareSplitHorizontal";
  import SquareSplitVertical from "phosphor-svelte/lib/SquareSplitVertical";
  import Stop from "phosphor-svelte/lib/Stop";
  import X from "phosphor-svelte/lib/X";
  import { onMount, tick } from "svelte";
  import { MediaQuery, SvelteSet } from "svelte/reactivity";
  import { api, errorText, type EventRow, type GitChange, type SessionDetail, type SessionInfo, type TerminalInfo, type Usage } from "$lib/api";
  import BranchPanel from "$lib/BranchPanel.svelte";
  import ChangesPanel from "$lib/ChangesPanel.svelte";
  import { openMenu, type MenuEntry } from "$lib/context-menu.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import FilesPanel from "$lib/FilesPanel.svelte";
  import FileViewer from "$lib/FileViewer.svelte";
  import { refocus } from "$lib/focus";
  import NeedsYou from "$lib/NeedsYou.svelte";
  import Terminal from "$lib/Terminal.svelte";
  import TerminalForm from "$lib/TerminalForm.svelte";
  import Timeline from "$lib/Timeline.svelte";
  import { CLI_LABEL, SIGNAL_TEXT, clock, duration, folderName, isLive, memory, modeLabel, runsCli, shortPath } from "$lib/format";
  import { plural, t, tb, type Key } from "$lib/i18n.svelte";
  import { MAX_PANES, PaneLayout, TERM, borders, rects, type PaneBorder, type PaneDir, type PaneDrop, type PaneRect, type PaneSide, type PaneTab } from "$lib/panes.svelte";
  import { pasteKey } from "$lib/platform";
  import { SessionShells } from "$lib/shells.svelte";
  import { app, showToast } from "$lib/store.svelte";
  import { GitWatch, splitPath, type ViewTarget } from "$lib/workspace.svelte";

  const id = $derived(page.url.searchParams.get("id") ?? "");

  let detail = $state<SessionDetail | null>(null);
  let events = $state<EventRow[]>([]);
  let loadState = $state<"loading" | "ready" | "missing" | "error">("loading");
  let loadError = $state("");
  let stopOpen = $state(false);
  let followUp = $state("");
  let sendError = $state("");
  let sending = $state(false);
  let resuming = $state(false);
  let termKey = $state(0);

  // The panel is the right column at every width. Hiding it gives the terminal the whole row, and that choice is
  // remembered on this computer. Below 720 both do not fit, so the panel opens over the terminal's right side instead
  // and closes again with its button, Escape, or a file opened from it.
  const HIDDEN_KEY = "oc-session-side-hidden";
  const narrow = new MediaQuery("max-width: 720px", false);
  let sideHidden = $state(readHidden());
  let sidePeek = $state(false);
  let sideBtn: HTMLButtonElement | undefined = $state();
  const sideOpen = $derived(narrow.current ? sidePeek : !sideHidden);

  function readHidden() {
    try {
      return localStorage.getItem(HIDDEN_KEY) === "1";
    } catch {
      return false;
    }
  }

  function toggleSide() {
    if (narrow.current) {
      sidePeek = !sidePeek;
      return;
    }
    sideHidden = !sideHidden;
    try {
      if (sideHidden) localStorage.setItem(HIDDEN_KEY, "1");
      else localStorage.removeItem(HIDDEN_KEY);
    } catch {
      // Private windows can refuse storage; the change still holds for this visit.
    }
  }

  // Escape closes the covering panel from inside it or from its button. The terminal keeps its own Escape, and a
  // dialog or a search that used Escape first keeps the panel open.
  function closePeek(e: KeyboardEvent) {
    if (e.key !== "Escape" || !sidePeek || e.defaultPrevented) return;
    const at = e.target as Element | null;
    if (!at || at.closest("dialog") || !(at === sideBtn || at.closest("#session-side"))) return;
    sidePeek = false;
    sideBtn?.focus();
  }

  $effect(() => {
    if (!narrow.current) sidePeek = false;
  });

  // The side panel's tabs. The one shown last is remembered on this computer; a tab's panel is
  // built the first time it is shown and then kept, so a folder opened in Files stays open.
  type SideTab = "files" | "changes" | "branch" | "details";
  const SIDE_TABS: { id: SideTab; label: Key; icon: typeof Files }[] = [
    { id: "files", label: "workspace.tab.files", icon: Files },
    { id: "changes", label: "workspace.tab.changes", icon: GitDiff },
    { id: "branch", label: "workspace.tab.branch", icon: GitBranch },
    { id: "details", label: "workspace.tab.details", icon: Info },
  ];
  const TAB_KEY = "oc-session-side-tab";
  function savedTab(): SideTab {
    try {
      const v = localStorage.getItem(TAB_KEY);
      return SIDE_TABS.some((x) => x.id === v) ? (v as SideTab) : "files";
    } catch {
      return "files";
    }
  }
  const firstTab = savedTab();
  let sideTab = $state<SideTab>(firstTab);
  const built = new SvelteSet<SideTab>([firstTab]);

  function pickTab(tab: SideTab) {
    sideTab = tab;
    built.add(tab);
    try {
      localStorage.setItem(TAB_KEY, tab);
    } catch {
      // Private windows can refuse storage; the tab still changes.
    }
  }

  // Left and Right move between the tabs, as in any tab list; the panel follows at once.
  function tabKeys(e: KeyboardEvent) {
    const i = SIDE_TABS.findIndex((x) => x.id === sideTab);
    const n = SIDE_TABS.length;
    const next = { ArrowRight: (i + 1) % n, ArrowLeft: (i - 1 + n) % n, Home: 0, End: n - 1 }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    pickTab(SIDE_TABS[next].id);
    document.getElementById(`side-tab-${SIDE_TABS[next].id}`)?.focus();
  }

  // Tabs above the terminal: the session's own terminal ("term") and plain shells opened in its folder (by their id),
  // each tab holding one of them or several split, and a file or change opened from the panel ("viewer").
  let viewer = $state<ViewTarget | null>(null);
  // A terminal of the tab to show, or "viewer".
  let mainTab = $state(TERM);
  const shells = $derived(new SessionShells(id));
  $effect(() => shells.start());
  const layout = $derived(new PaneLayout(id, () => shells.list.map((x) => x.id)));
  // Named by its first terminal. A tab that went away (a closed shell, a new session) falls back to the session's own.
  const shownTab = $derived(mainTab === "viewer" && viewer ? "viewer" : (layout.tabOf(mainTab)?.panes[0] ?? TERM));
  const shown: PaneTab | null = $derived(shownTab === "viewer" ? null : (layout.tabOf(shownTab) ?? null));
  const split = $derived((shown?.panes.length ?? 0) > 1);
  const inSplit = $derived(new Set(layout.tabs.filter((x) => x.panes.length > 1).flatMap((x) => x.panes)));
  const canSplit = $derived(shells.state === "ready" && shown !== null && shown.panes.length < MAX_PANES);
  // The way the shown tab's outer split runs; Split adds a terminal that way.
  const shownDir: PaneDir = $derived(shown?.root.kind === "split" ? shown.root.dir : "row");
  // Each terminal's place in the shown tab, and the borders between them, each after the terminals before it.
  const places = $derived(shown ? rects(shown.root) : new Map<string, PaneRect>());
  const bordersAfter = $derived.by(() => {
    const out = new Map<string, PaneBorder[]>();
    for (const b of shown ? borders(shown.root) : []) {
      const last = b.before[b.before.length - 1];
      out.set(last, [...(out.get(last) ?? []), b]);
    }
    return out;
  });
  const pct = (n: number) => `${+(n * 100).toFixed(4)}%`;
  let newShellOpen = $state(false);
  // The tab a New terminal dialog opened from Split puts its shell in.
  let splitInto = $state<string | null>(null);
  let restarting = $state("");
  let shellFailure = $state("");

  const paneName = (p: string) => (p === TERM ? (s ? CLI_LABEL[s.cli] : "") : (shells.names.get(p) ?? ""));
  const panelId = (p: string) => (p === TERM ? "session-term" : `shell-panel-${p}`);
  const tabButtonId = (p: string) => (p === TERM ? "view-tab-term" : `shell-tab-${p}`);

  function newShell(into: string | null) {
    splitInto = into;
    newShellOpen = true;
  }

  async function openShell(shell: string) {
    const into = splitInto;
    const info = await shells.open(shell, into ? (x) => layout.split(into, x) : undefined);
    mainTab = info.id;
    newShellOpen = false;
  }

  async function closeShell(x: TerminalInfo) {
    shellFailure = "";
    const tabs = layout.tabs;
    const at = tabs.findIndex((y) => y.panes.includes(x.id));
    const rest = tabs[at]?.panes.filter((p) => p !== x.id) ?? [];
    try {
      await shells.close(x.id);
    } catch (e) {
      shellFailure = errorText(e);
      return;
    }
    // A split tab keeps its other terminals; a tab's only terminal gives way to the tab after it, or the one before.
    const next = rest[0] ?? layout.tabs[Math.min(at, layout.tabs.length - 1)]?.panes[0] ?? TERM;
    if (mainTab === x.id) mainTab = next;
    // The button that closed it is gone: focus goes to the tab left in its place.
    await tick();
    document.getElementById(tabButtonId(next))?.focus();
  }

  // A border between split terminals moves with the pointer, or with the arrow keys along it (Home and End go as far
  // as they can). No terminal gets narrower, or lower, than MIN_PANE.
  const MIN_PANE = 120;
  const STEP = 0.05;
  let panesBox: HTMLDivElement | undefined = $state();

  /** The length in pixels of the split a border belongs to, along its direction. */
  function splitLength(b: PaneBorder) {
    const r = panesBox?.getBoundingClientRect();
    return r ? (b.dir === "row" ? r.width * b.box.w : r.height * b.box.h) : 0;
  }

  /** The share of its split that the part before border `b` has now. */
  function partSize(b: PaneBorder) {
    let node = shown?.root;
    for (const j of b.path) node = node?.kind === "split" ? node.children[j] : node;
    return node?.kind === "split" ? node.sizes[b.i] : 0;
  }

  function borderKeys(e: KeyboardEvent, b: PaneBorder) {
    const row = b.dir === "row";
    const size = partSize(b);
    const keys: Record<string, number> = {
      [row ? "ArrowLeft" : "ArrowUp"]: size - STEP,
      [row ? "ArrowRight" : "ArrowDown"]: size + STEP,
      Home: 0,
      End: 1,
    };
    if (!(e.key in keys)) return;
    e.preventDefault();
    const length = splitLength(b);
    layout.resize(shownTab, b.path, b.i, keys[e.key], length > 0 ? MIN_PANE / length : 0.1);
  }

  /**
   * Follows the pointer on the whole window until it is let go, whatever it passes over (a terminal, the tabs).
   * `cursor` is shown everywhere meanwhile, and no text is selected.
   */
  function follow(cursor: string, move: (e: PointerEvent) => void, end: (e: PointerEvent | null) => void) {
    const root = document.documentElement;
    const stop = (e: PointerEvent | null) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      removeEventListener("pointercancel", cancel);
      removeEventListener("keydown", esc, true);
      delete root.dataset.paneDrag;
      end(e);
    };
    const up = (e: PointerEvent) => stop(e);
    const cancel = () => stop(null);
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      stop(null);
    };
    root.dataset.paneDrag = cursor;
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
    addEventListener("pointercancel", cancel);
    addEventListener("keydown", esc, true);
  }

  function borderDrag(e: PointerEvent, b: PaneBorder) {
    if (e.button !== 0) return;
    const lead = shownTab;
    const length = splitLength(b);
    if (length <= 0) return;
    e.preventDefault();
    const at = (p: PointerEvent) => (b.dir === "row" ? p.clientX : p.clientY);
    const from = at(e);
    const start = partSize(b);
    follow(
      b.dir === "row" ? "col-resize" : "row-resize",
      (m) => layout.resize(lead, b.path, b.i, start + (at(m) - from) / length, MIN_PANE / length),
      (u) => {
        // Escape puts the border back where it was.
        if (!u) layout.resize(lead, b.path, b.i, start, 0);
      },
    );
  }

  // A terminal is dragged by its tab (a split tab moves as a whole) or, in a split tab, by its header. It lands between
  // tabs, on a tab to join it, or beside a terminal of the shown tab. The drag starts after 5 px, so a click still picks
  // the tab; the context menu offers the same moves without dragging.
  type Drag = { pane: string; whole: boolean; name: string; x: number; y: number };
  let drag = $state<Drag | null>(null);
  let drop = $state<PaneDrop | null>(null);
  // A drag that ends over the control it started on is not a click on it.
  let dragged = false;

  function dragFrom(e: PointerEvent, pane: string, whole: boolean) {
    if (e.button !== 0) return;
    const x0 = e.clientX;
    const y0 = e.clientY;
    const name = whole ? (layout.tabOf(pane)?.panes.map(paneName).join(", ") ?? "") : paneName(pane);
    follow(
      "grabbing",
      (m) => {
        if (!drag && Math.hypot(m.clientX - x0, m.clientY - y0) < 5) return;
        getSelection()?.removeAllRanges();
        drag = { pane, whole, name, x: m.clientX, y: m.clientY };
        drop = dropAt(drag, m.clientX, m.clientY);
      },
      (u) => {
        if (!drag) return;
        if (u && drop) {
          if (drop.kind === "tab" && drag.whole) layout.moveTab(pane, drop.at);
          else layout.move(pane, drop);
          mainTab = pane;
        }
        drag = null;
        drop = null;
        dragged = true;
        setTimeout(() => (dragged = false));
      },
    );
  }

  /** Where the drag would land at this point, or null when it cannot land there. */
  function dropAt(d: Drag, x: number, y: number): PaneDrop | null {
    const at = document.elementFromPoint(x, y);
    const fits = (to: PaneDrop) => (d.whole && layout.tabOf(d.pane)!.panes.length > 1 ? to.kind === "tab" : layout.canMove(d.pane, to));
    const tabs = layout.tabs;
    const tabEl = at?.closest<HTMLElement>("[data-tab]");
    if (tabEl) {
      const lead = tabEl.dataset.tab ?? "";
      const i = tabs.findIndex((t) => t.panes[0] === lead);
      const r = tabEl.getBoundingClientRect();
      const f = r.width > 0 ? (x - r.left) / r.width : 0.5;
      const join: PaneDrop = { kind: "join", tab: lead };
      if (f > 0.25 && f < 0.75 && fits(join)) return join;
      return { kind: "tab", at: f < 0.5 ? i : i + 1 };
    }
    if (at?.closest("#session-views")) return { kind: "tab", at: tabs.length };
    const paneEl = at?.closest<HTMLElement>("#session-panes [data-pane]");
    if (!paneEl || !shown) return null;
    const r = paneEl.getBoundingClientRect();
    const fx = (x - r.left) / (r.width || 1) - 0.5;
    const fy = (y - r.top) / (r.height || 1) - 0.5;
    // The terminal's nearest edge is the side it lands on.
    const side: PaneSide = Math.abs(fx) >= Math.abs(fy) ? (fx < 0 ? "left" : "right") : fy < 0 ? "top" : "bottom";
    const to: PaneDrop = { kind: "beside", pane: paneEl.dataset.pane ?? "", side };
    return fits(to) ? to : null;
  }

  /** The tab before which a drop puts its terminal: a mark on that tab's left edge, or on the last tab's right one. */
  const dropBefore = $derived(drop?.kind === "tab" ? (layout.tabs[drop.at]?.panes[0] ?? null) : null);
  const dropLast = $derived(drop?.kind === "tab" && drop.at >= layout.tabs.length);
  const dropSide = (p: string) => (drop?.kind === "beside" && drop.pane === p ? drop.side : "");

  // The same moves from the keyboard or without dragging: right-click (Shift+F10, the Menu key) on a tab, or the
  // header of a split terminal.
  function moved(pane: string, move: () => void) {
    const from = document.activeElement as HTMLElement | null;
    move();
    mainTab = pane;
    // Moving an element in the page takes its focus away; a control that went with the move gives way to the tab.
    tick().then(() => {
      if (from?.isConnected) from.focus();
      else document.getElementById(tabButtonId(layout.tabOf(pane)?.panes[0] ?? TERM))?.focus();
    });
  }

  function moveMenu(e: MouseEvent, pane: string, whole: boolean) {
    const tabs = layout.tabs;
    const ti = tabs.findIndex((t) => t.panes.includes(pane));
    const tab = tabs[ti];
    // One tab alone has nowhere to go.
    if (dragged || !tab || (whole && tabs.length === 1)) {
      e.preventDefault();
      return;
    }
    const items: MenuEntry[] = [];
    if (whole) {
      items.push(
        { label: t("terminal.menu.tabLeft"), icon: ArrowLeft, disabled: ti === 0, action: () => moved(pane, () => layout.moveTab(pane, ti - 1)) },
        { label: t("terminal.menu.tabRight"), icon: ArrowRight, disabled: ti === tabs.length - 1, action: () => moved(pane, () => layout.moveTab(pane, ti + 2)) },
      );
    } else {
      for (const o of tab.panes) {
        if (o !== pane) items.push({ label: t("terminal.menu.swap", { name: paneName(o) }), icon: ArrowsLeftRight, action: () => moved(pane, () => layout.swap(pane, o)) });
      }
      const edges: [PaneSide, Key, typeof ArrowLeft][] = [
        ["left", "terminal.menu.left", ArrowLeft],
        ["right", "terminal.menu.right", ArrowRight],
        ["top", "terminal.menu.up", ArrowUp],
        ["bottom", "terminal.menu.down", ArrowDown],
      ];
      for (const [side, label, icon] of edges) items.push({ label: t(label), icon, action: () => moved(pane, () => layout.dock(pane, side)) });
      items.push({ label: t("terminal.menu.newTab"), icon: Plus, action: () => moved(pane, () => layout.move(pane, { kind: "tab", at: ti + 1 })) });
    }
    // A terminal on its own, or one of a split, can join another tab.
    if (!whole || tab.panes.length === 1) {
      const others = tabs.filter((_, j) => j !== ti);
      if (others.length) items.push(null);
      for (const o of others) {
        const full = o.panes.length >= MAX_PANES;
        items.push({
          label: t("terminal.menu.join", { tab: paneName(o.panes[0]) }),
          icon: SquareSplitHorizontal,
          disabled: full,
          hint: full ? t("terminal.splitFull", { n: MAX_PANES }) : undefined,
          action: () => moved(pane, () => layout.move(pane, { kind: "join", tab: o.panes[0] })),
        });
      }
    }
    openMenu(e, `terminal:${pane}`, t("terminal.menu.label", { name: whole ? paneName(tab.panes[0]) : paneName(pane) }), items);
  }

  // A split tab shows one keyboard note below its terminals, for the one last focused.
  let focusedPane = $state("");
  function notePane(e: FocusEvent) {
    const p = (e.target as Element | null)?.closest<HTMLElement>("[data-pane]")?.dataset.pane;
    if (p) focusedPane = p;
  }
  const notedPane = $derived(shown && split ? (shown.panes.includes(focusedPane) ? focusedPane : shown.panes[0]) : "");

  /** The note under a running terminal; a closed one has its own row instead. */
  function liveNote(p: string): string {
    if (p === TERM) {
      if (!s || s.mode !== "interactive" || !live) return "";
      return t("sessions.detail.terminalLive", { paste: pasteKey(), cli: CLI_LABEL[s.cli], command: s.cli });
    }
    return shells.list.find((x) => x.id === p)?.running ? t("terminal.live", { paste: pasteKey() }) : "";
  }

  async function restartShell(x: TerminalInfo) {
    shellFailure = "";
    restarting = x.id;
    try {
      await shells.restart(x.id);
    } catch (e) {
      shellFailure = errorText(e);
    } finally {
      restarting = "";
    }
  }

  function show(target: ViewTarget) {
    viewer = target;
    mainTab = "viewer";
    // The panel covers the viewer when it opens over the terminal.
    sidePeek = false;
  }

  function closeViewer() {
    viewer = null;
    mainTab = TERM;
    refocus(document.getElementById("session-term"));
  }

  const watch = $derived(new GitWatch(id));
  // Git is read while a shown tab or the viewer shows what it says, every 5 s while the window is in view.
  const readsGit = $derived((sideOpen && sideTab !== "details") || viewer !== null);

  async function load(target: string) {
    loadState = "loading";
    detail = null;
    events = [];
    viewer = null;
    mainTab = TERM;
    if (!target) {
      loadState = "missing";
      return;
    }
    try {
      const d = await api.getSession(target);
      if (target !== id) return;
      detail = d;
      // Events that arrived while the page loaded are kept; the stored ones come first.
      const known = new Set(d.events.map((e) => e.id));
      events = [...d.events, ...events.filter((e) => !known.has(e.id))];
      loadState = "ready";
    } catch (e) {
      // A slow failure for a session left behind does not replace the one now shown.
      if (target !== id) return;
      const msg = errorText(e);
      if (msg === "Session not found.") loadState = "missing";
      else {
        loadState = "error";
        loadError = msg;
      }
    }
  }

  $effect(() => {
    load(id);
  });

  onMount(() => {
    const un = listen<EventRow>("session-event", (e) => {
      if (e.payload.sessionId === id && !events.some((x) => x.id === e.payload.id)) {
        events = [...events, e.payload];
        if (e.payload.event.kind === "file_changed" && readsGit) watch.refresh();
      }
    });
    return () => {
      un.then((f) => f());
    };
  });

  // The store receives every status change; fall back to the loaded copy.
  const s: SessionInfo | null = $derived(app.sessions.find((x) => x.id === id) ?? detail?.session ?? null);
  const live = $derived(s ? isLive(s) : false);
  const version = $derived(s ? app.clis.find((c) => c.kind === s.cli)?.version : null);
  const files = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of events) if (e.event.kind === "file_changed") counts.set(e.event.path, (counts.get(e.event.path) ?? 0) + 1);
    return [...counts.entries()];
  });

  // CPU and memory of the CLI and everything it started (PRD FR-34), measured while it runs.
  let usage = $state<Usage | null>(null);
  let measuring = $state(false);

  $effect(() => {
    const target = id;
    usage = null;
    measuring = live;
    if (!live) return;
    let gone = false;
    const measure = async () => {
      try {
        const u = await api.sessionUsage(target);
        if (gone) return;
        usage = u;
        measuring = false;
      } catch {
        if (!gone) measuring = false;
      }
    };
    measure();
    const timer = setInterval(measure, 3000);
    return () => {
      gone = true;
      clearInterval(timer);
    };
  });

  $effect(() => {
    const w = watch;
    if (!readsGit || loadState !== "ready") return;
    w.refresh();
    const timer = setInterval(() => {
      if (document.visibilityState !== "hidden") w.refresh();
    }, 5000);
    return () => clearInterval(timer);
  });

  const changeCount = $derived(watch.status?.changes.length ?? 0);

  function openChange(c: GitChange) {
    show({ kind: "diff", path: c.path });
  }

  const canFollowUp = $derived.by(() => {
    if (!s || s.mode !== "headless") return false;
    if (s.status === "waiting") return false;
    if (!live) return Boolean(s.cliSessionId);
    return s.cli === "claude";
  });
  const followUpHint = $derived.by(() => {
    if (!s) return "";
    if (s.status === "waiting") return t("sessions.detail.hintAnswerFirst");
    if (live && s.cli !== "claude") return t("sessions.detail.hintBusy", { cli: CLI_LABEL[s.cli] });
    if (!live && !s.cliSessionId) return t("sessions.detail.hintNoId", { cli: CLI_LABEL[s.cli] });
    return t("sessions.detail.hintFollowUp", { cli: CLI_LABEL[s.cli] });
  });

  async function sendFollowUp(e: SubmitEvent) {
    e.preventDefault();
    const text = followUp.trim();
    if (!text || !s) return;
    sending = true;
    sendError = "";
    try {
      await api.sendInput(s.id, text);
      followUp = "";
    } catch (err) {
      sendError = errorText(err);
    } finally {
      sending = false;
    }
  }

  async function confirmStop() {
    if (!s) return;
    stopOpen = false;
    try {
      await api.stopSession(s.id);
      showToast(t("sessions.detail.stopping", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) }));
    } catch (err) {
      showToast(tb(errorText(err)));
    }
  }

  // A terminal session's shell outlives its CLI, so it closes only with `exit`; this opens it again,
  // with the CLI's own history when it has one.
  async function resume() {
    if (!s) return;
    resuming = true;
    try {
      await api.resumeSession(s.id, 120, 32);
      termKey += 1;
      detail = await api.getSession(s.id);
    } catch (err) {
      showToast(tb(errorText(err)));
    } finally {
      resuming = false;
    }
  }
</script>

<svelte:head><title>{s ? s.title : t("sessions.detail.pageTitle")} · OpenCompanion</title></svelte:head>
<svelte:window onkeydown={closePeek} />

<main class="main detail" id="session-main">
  {#if loadState === "loading"}
    <p class="hint" role="status">{t("sessions.detail.loading")}</p>
  {:else if loadState === "missing" || !s}
    <div class="not-found">
      <p class="crumb"><a href="/">{t("sessions.overview.title")}</a></p>
      <h1>{t("sessions.detail.notFoundTitle")}</h1>
      <p class="sub">{t("sessions.detail.notFoundBody")}</p>
      <a class="btn secondary" href="/">{t("sessions.backToOverview")}</a>
    </div>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("sessions.detail.loadFailed")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={() => load(id)}>{t("sessions.tryAgain")}</button>
    </div>
  {:else}
    <!-- The sidebar names the session and its status; the rest is under Details. -->
    <h1 class="sr-only">{s.title}</h1>

    {#if s.status === "waiting"}
      <NeedsYou {s} compact showOpen={false} />
    {/if}

    <div class="detail-body ws" class:solo={!sideOpen}>
      <div class="main-pane">
        <div class="views-bar">
          <div class="tab-strip" id="session-views" role="group" aria-label={t("workspace.viewer.tabs")}>
            {#each layout.tabs as tab (tab.panes[0])}
              {@const lead = tab.panes[0]}
              {@const more = tab.panes.length - 1}
              {@const x = shells.list.find((y) => y.id === lead)}
              <div
                class="tab"
                class:current={shownTab === lead}
                class:drop-before={dropBefore === lead}
                class:drop-after={dropLast && lead === layout.tabs.at(-1)?.panes[0]}
                class:drop-join={drop?.kind === "join" && drop.tab === lead}
                data-tab={lead}
              >
                <button
                  class="tab-pick"
                  type="button"
                  id={tabButtonId(lead)}
                  aria-current={shownTab === lead ? "true" : undefined}
                  aria-controls={tab.panes.map(panelId).join(" ")}
                  title={more ? `${tab.panes.map(paneName).join(", ")}\n${t("terminal.dragHint")}` : t("terminal.dragHint")}
                  onclick={() => dragged || (mainTab = lead)}
                  onpointerdown={(e) => dragFrom(e, lead, true)}
                  oncontextmenu={(e) => moveMenu(e, lead, true)}
                >
                  <b>{paneName(lead)}</b>
                  {#if !x}
                    <small>{s.mode === "interactive" ? t("workspace.viewer.terminal") : t("workspace.viewer.output")}</small>
                  {:else if !x.running}
                    <span class="chip idle">{t("terminal.exited")}</span>
                  {/if}
                  {#if more}
                    <small class="tab-more" aria-hidden="true">+{more}</small>
                    <span class="sr-only">{" "}{plural(more, "terminal.moreOne", "terminal.moreMany")}</span>
                  {/if}
                </button>
                <!-- A split tab has no close of its own: each of its terminals closes from its header. -->
                {#if x && !more}
                  <button class="icon-btn tab-close" type="button" aria-label={t("terminal.closeNamed", { name: paneName(lead) })} title={t("terminal.closeHint", { shell: x.shellLabel })} onclick={() => closeShell(x)}>
                    <X size={14} aria-hidden="true" />
                  </button>
                {/if}
              </div>
            {/each}
            {#if viewer}
              {@const name = splitPath(viewer.path).name}
              <div class="tab" class:current={shownTab === "viewer"}>
                <button class="tab-pick" type="button" id="view-tab-file" aria-current={shownTab === "viewer" ? "true" : undefined} aria-controls="session-viewer" title={viewer.path} onclick={() => (mainTab = "viewer")}>
                  <b class="mono">{name}</b>
                  <small>{viewer.kind === "diff" ? t("workspace.viewer.kindDiff") : t("workspace.viewer.kindFile")}</small>
                </button>
                <button class="icon-btn tab-close" type="button" aria-label={t("workspace.viewer.close", { name })} title={t("workspace.viewer.close", { name })} onclick={closeViewer}>
                  <X size={14} aria-hidden="true" />
                </button>
              </div>
            {/if}
            <button class="icon-btn tab-new" type="button" id="btn-new-terminal" aria-label={t("terminal.new")} title={t("terminal.newIn", { folder: folderName(s.cwd) })} onclick={() => newShell(null)}>
              <Plus size={16} aria-hidden="true" />
            </button>
          </div>
          <!-- Controls for the shown tab's terminals; a file or a change has none. -->
          {#if shown}
            <button
              class="icon-btn bar-btn"
              type="button"
              id="btn-split-terminal"
              aria-label={t("terminal.split")}
              title={shown.panes.length >= MAX_PANES ? t("terminal.splitFull", { n: MAX_PANES }) : shownDir === "column" ? t("terminal.splitBelow") : t("terminal.splitBeside")}
              disabled={!canSplit}
              onclick={() => newShell(shownTab)}
            >
              {#if shownDir === "column"}<SquareSplitVertical size={20} aria-hidden="true" />{:else}<SquareSplitHorizontal size={20} aria-hidden="true" />{/if}
            </button>
            {#if split}
              <button
                class="icon-btn bar-btn"
                type="button"
                id="btn-stack-terminals"
                aria-label={t("terminal.stack")}
                aria-pressed={shownDir === "column"}
                title={shownDir === "column" ? t("terminal.sideHint") : t("terminal.stackHint")}
                onclick={() => layout.flip(shownTab)}
              >
                <Rows size={20} weight={shownDir === "column" ? "fill" : "regular"} aria-hidden="true" />
              </button>
            {/if}
          {/if}
          <button
            class="icon-btn bar-btn"
            type="button"
            id="btn-side-panel"
            bind:this={sideBtn}
            aria-expanded={sideOpen}
            aria-controls="session-side"
            aria-label={t("workspace.toggle")}
            title={sideOpen ? t("workspace.hide") : t("workspace.show")}
            onclick={toggleSide}
          >
            <SidebarSimple size={20} weight={sideOpen ? "fill" : "regular"} mirrored aria-hidden="true" />
          </button>
        </div>
        {#if shells.state === "error"}
          <p class="err-text shell-note" role="alert">
            <span>{t("terminal.loadFailed", { error: tb(shells.error) })}</span>
            <button class="btn secondary sm" type="button" onclick={() => shells.load()}>{t("sessions.tryAgain")}</button>
          </p>
        {/if}
        {#if shellFailure}<p class="err-text shell-note" role="alert">{tb(shellFailure)}</p>{/if}
        <!-- Every terminal stays mounted in one dark panel, in tab order, so moving one never restarts it; the shown tab's
             terminals are the visible ones. -->
        {#snippet paneHead(p: string, x: TerminalInfo | null)}
          {#if inSplit.has(p)}
            <div class="pane-head">
              <button
                class="pane-name"
                type="button"
                aria-haspopup="menu"
                title={t("terminal.paneHint")}
                onpointerdown={(e) => dragFrom(e, p, false)}
                onclick={(e) => moveMenu(e, p, false)}
                oncontextmenu={(e) => moveMenu(e, p, false)}
              >
                <b>{paneName(p)}</b>
                {#if !x}<small>{s?.mode === "interactive" ? t("workspace.viewer.terminal") : t("workspace.viewer.output")}</small>{/if}
                <CaretDown size={12} aria-hidden="true" />
              </button>
              {#if x && !x.running}<span class="chip idle">{t("terminal.exited")}</span>{/if}
              {#if x}
                <button class="icon-btn pane-close" type="button" aria-label={t("terminal.closeNamed", { name: paneName(p) })} title={t("terminal.closeHint", { shell: x.shellLabel })} onclick={() => closeShell(x)}>
                  <X size={14} aria-hidden="true" />
                </button>
              {/if}
            </div>
          {/if}
          {#if dropSide(p)}<div class="drop-zone {dropSide(p)}" aria-hidden="true"></div>{/if}
        {/snippet}
        <div class="term joined" id="session-panes">
          <div class="panes" bind:this={panesBox} onfocusin={notePane}>
            {#each layout.tabs.flatMap((tab) => tab.panes) as p (p)}
              {@const x = shells.list.find((y) => y.id === p) ?? null}
              {@const r = places.get(p)}
              {#if p === TERM}
                <section
                  class="pane"
                  id="session-term"
                  data-pane={TERM}
                  hidden={!r}
                  style:left={pct(r?.x ?? 0)}
                  style:top={pct(r?.y ?? 0)}
                  style:width={pct(r?.w ?? 1)}
                  style:height={pct(r?.h ?? 1)}
                  aria-label={t("sessions.detail.output")}
                >
                  {@render paneHead(TERM, null)}
                  {#if s.mode === "interactive"}
                    {#key `${id}-${termKey}`}
                      <Terminal id={s.id} {live} label={t("sessions.detail.terminalLabel", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })} />
                    {/key}
                    {#if live}
                      {#if !inSplit.has(TERM)}<div class="term-note">{liveNote(TERM)}</div>{/if}
                    {:else}
                      <div class="term-note exit-row">
                        <span class="grow">{t("sessions.detail.terminalClosed")}</span>
                        <button class="btn primary sm" type="button" id="btn-open-terminal-again" disabled={resuming} onclick={resume}>
                          <Play size={14} aria-hidden="true" />{resuming ? t("sessions.detail.opening") : t("sessions.detail.openAgain")}
                        </button>
                      </div>
                    {/if}
                  {:else}
                    <Timeline {events} />
                    <form class="term-in" onsubmit={sendFollowUp}>
                      <label for="term-input">›</label>
                      <input id="term-input" bind:value={followUp} placeholder={followUpHint} disabled={!canFollowUp || sending} autocomplete="off" spellcheck="false" />
                      <small>{canFollowUp ? t("sessions.detail.enterToSend") : ""}</small>
                      {#if live}
                        <button class="btn danger sm" type="button" onclick={() => (stopOpen = true)}><Stop size={14} aria-hidden="true" />{t("sessions.detail.stop")}</button>
                      {/if}
                    </form>
                    {#if sendError}<div class="term-note" role="alert">{tb(sendError)}</div>{/if}
                  {/if}
                </section>
              {:else if x}
                {@const label = t("terminal.label", { shell: shells.names.get(x.id) ?? x.shellLabel, folder: folderName(s.cwd) })}
                <section
                  class="pane"
                  id="shell-panel-{x.id}"
                  data-pane={x.id}
                  aria-label={label}
                  hidden={!r}
                  style:left={pct(r?.x ?? 0)}
                  style:top={pct(r?.y ?? 0)}
                  style:width={pct(r?.w ?? 1)}
                  style:height={pct(r?.h ?? 1)}
                >
                  {@render paneHead(x.id, x)}
                  {#key `${x.id}-${x.startedAt}`}
                    <Terminal kind="terminal" id={x.id} live={x.running} {label} />
                  {/key}
                  {#if x.running}
                    {#if !inSplit.has(x.id)}<div class="term-note">{liveNote(x.id)}</div>{/if}
                  {:else}
                    <div class="term-note exit-row">
                      <span class="grow">
                        {x.exitCode === null ? t("terminal.exitedNoCode", { shell: x.shellLabel }) : t("terminal.exitedCode", { shell: x.shellLabel, code: x.exitCode })}
                      </span>
                      <button class="btn primary sm" type="button" disabled={restarting === x.id} onclick={() => restartShell(x)}>
                        <ArrowClockwise size={14} aria-hidden="true" />{restarting === x.id ? t("terminal.restarting") : t("terminal.restart")}
                      </button>
                      <button class="btn secondary sm" type="button" onclick={() => closeShell(x)}>
                        <X size={14} aria-hidden="true" />{t("shell.close")}
                      </button>
                    </div>
                  {/if}
                </section>
              {/if}
              {#each bordersAfter.get(p) ?? [] as b (`${b.path.join(".")}:${b.i}`)}
                {@const row = b.dir === "row"}
                <!-- A focusable separator is ARIA's window splitter, a widget; Svelte's check counts every separator as static. -->
                <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
                <div
                  class="pane-border"
                  class:across={!row}
                  role="separator"
                  tabindex="0"
                  style:left={pct(row ? b.at : b.box.x)}
                  style:top={pct(row ? b.box.y : b.at)}
                  style:width={row ? undefined : pct(b.box.w)}
                  style:height={row ? pct(b.box.h) : undefined}
                  aria-orientation={row ? "vertical" : "horizontal"}
                  aria-controls={b.before.map(panelId).join(" ")}
                  aria-label={t("terminal.resize", { first: b.before.map(paneName).join(", "), second: b.after.map(paneName).join(", ") })}
                  aria-valuenow={Math.round(b.share * 100)}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  onkeydown={(e) => borderKeys(e, b)}
                  onpointerdown={(e) => borderDrag(e, b)}
                ></div>
              {/each}
            {/each}
            {#if viewer}
              <section class="pane whole" id="session-viewer" hidden={shownTab !== "viewer"}>
                <FileViewer
                  {id}
                  target={viewer}
                  change={watch.change(viewer.path)}
                  version={watch.version}
                  onkind={(kind) => viewer && (viewer = { kind, path: viewer.path })}
                />
              </section>
            {/if}
          </div>
          {#if notedPane && liveNote(notedPane)}<div class="term-note">{liveNote(notedPane)}</div>{/if}
        </div>
      </div>

      <aside class="detail-side tabbed" id="session-side" hidden={!sideOpen}>
        <div class="side-tabs" role="tablist" aria-label={t("workspace.panel")} tabindex="-1" onkeydown={tabKeys}>
          {#each SIDE_TABS as tab (tab.id)}
            <button
              class="side-tab"
              type="button"
              role="tab"
              id="side-tab-{tab.id}"
              aria-selected={sideTab === tab.id}
              aria-controls="side-panel-{tab.id}"
              aria-label={tab.id === "changes" && changeCount > 0 ? plural(changeCount, "workspace.tab.changesOne", "workspace.tab.changesMany") : undefined}
              tabindex={sideTab === tab.id ? 0 : -1}
              onclick={() => pickTab(tab.id)}
            >
              <span class="st-icon">
                <tab.icon size={18} aria-hidden="true" />
                {#if tab.id === "changes" && changeCount > 0}<span class="st-count" aria-hidden="true">{changeCount}</span>{/if}
              </span>
              <span class="st-label">{t(tab.label)}</span>
            </button>
          {/each}
        </div>
        {#key id}
          {#if built.has("files")}
            <div class="side-panel" role="tabpanel" id="side-panel-files" aria-labelledby="side-tab-files" hidden={sideTab !== "files"}>
              <FilesPanel
                {id}
                cwd={s.cwd}
                status={watch.status}
                version={watch.version}
                current={viewer?.path ?? null}
                onopen={(path, line) => show({ kind: "file", path, line })}
                onrefresh={() => watch.refresh()}
              />
            </div>
          {/if}
          {#if built.has("changes")}
            <div class="side-panel" role="tabpanel" id="side-panel-changes" aria-labelledby="side-tab-changes" hidden={sideTab !== "changes"}>
              <ChangesPanel {watch} cwd={s.cwd} current={viewer?.kind === "diff" ? viewer.path : null} onopen={openChange} />
            </div>
          {/if}
          {#if built.has("branch")}
            <div class="side-panel" role="tabpanel" id="side-panel-branch" aria-labelledby="side-tab-branch" hidden={sideTab !== "branch"}>
              <BranchPanel {id} cwd={s.cwd} {watch} busyCli={runsCli(s) ? CLI_LABEL[s.cli] : null} />
            </div>
          {/if}
        {/key}
        <div class="side-panel details" role="tabpanel" id="side-panel-details" aria-labelledby="side-tab-details" hidden={sideTab !== "details"}>
          <div class="side-group">
            <h3>{t("sessions.detail.filesChanged")}</h3>
            {#if files.length === 0}
              <p>{s.mode === "interactive" && s.cli !== "claude" ? t("sessions.detail.filesNotReported") : t("sessions.detail.noFiles")}</p>
            {:else}
              {#each files as [path, n] (path)}
                <div class="kv"><span class="mono" title={path}>{folderName(path)}</span><b class="mono">{n}×</b></div>
              {/each}
            {/if}
          </div>
          <div class="side-group">
            <h3>{t("sessions.detail.about")}</h3>
            <div class="kv"><span>{t("sessions.detail.cli")}</span><b>{CLI_LABEL[s.cli]}{version ? ` ${version}` : ""}</b></div>
            <div class="kv"><span>{t("sessions.detail.mode")}</span><b>{t(`sessions.mode.${s.mode}`)}</b></div>
            <div class="kv"><span>{t("sessions.detail.permissions")}</span><b>{modeLabel(s.permissionMode)}</b></div>
            <div class="kv"><span>{t("sessions.detail.folder")}</span><b class="mono" title={s.cwd}>{shortPath(s.cwd)}</b></div>
            <div class="kv"><span>{t("sessions.detail.started")}</span><b>{clock(s.startedAt)}</b></div>
            {#if s.source === "chat"}<div class="kv"><span>{t("sessions.detail.startedFrom")}</span><b>{t("sessions.detail.chat")}</b></div>{/if}
          </div>
          <div class="side-group">
            <h3>{t("sessions.process")}</h3>
            <div class="kv"><span>{t("sessions.detail.status")}</span><b>{t(`sessions.status.${s.status}`)}</b></div>
            {#if s.pid && live}<div class="kv"><span>{t("sessions.pid")}</span><b class="mono">{s.pid}</b></div>{/if}
            {#if live && usage}
              <div class="kv"><span>{t("sessions.cpu")}</span><b class="mono">{usage.cpuPercent}%</b></div>
              <div class="kv"><span>{t("sessions.memory")}</span><b class="mono">{memory(usage.memoryBytes)}</b></div>
              <div class="kv"><span>{t("sessions.detail.childProcesses")}</span><b class="mono">{usage.children}</b></div>
            {:else if measuring}
              <p>{t("sessions.detail.measuring")}</p>
            {/if}
            <div class="kv"><span>{t("sessions.runningFor")}</span><b>{duration((s.endedAt ?? app.now) - s.startedAt)}</b></div>
            {#if s.exitCode !== null}<div class="kv"><span>{t("sessions.detail.exitCode")}</span><b class="mono">{s.exitCode}</b></div>{/if}
            {#if s.cliSessionId}<div class="kv"><span>{t("sessions.detail.cliSession")}</span><b class="mono" title={s.cliSessionId}>{s.cliSessionId.slice(0, 12)}</b></div>{/if}
          </div>
          <div class="side-group">
            <h3>{t("sessions.detail.signal")}</h3>
            <p>
              {#if s.waiting}{SIGNAL_TEXT[s.waiting.method]}
              {:else if s.mode === "headless" && s.cli === "claude"}{SIGNAL_TEXT.stdio}
              {:else if s.mode === "interactive" && s.cli === "claude"}{SIGNAL_TEXT.hook}
              {:else if s.mode === "headless" && s.cli === "opencode"}{t("sessions.detail.signalOpenCode")}
              {:else if s.cli === "pi"}{t("sessions.detail.signalPi")}
              {:else if s.cli === "omp"}{t("sessions.detail.signalOmp")}
              {:else}{SIGNAL_TEXT.screen}{/if}
            </p>
          </div>
        </div>
      </aside>
    </div>
  {/if}
  {#if drag}<div class="drag-ghost" style:left="{drag.x}px" style:top="{drag.y}px" aria-hidden="true">{drag.name}</div>{/if}
</main>

<Dialog bind:open={newShellOpen} labelledby="nt-title">
  {#if s}
    <TerminalForm folder={s.cwd} split={splitInto !== null} onsubmit={openShell} oncancel={() => (newShellOpen = false)} />
  {/if}
</Dialog>

<Dialog bind:open={stopOpen} labelledby="stop-title">
  <div class="d-body">
    <h2 id="stop-title">{t("sessions.detail.stopTitle")}</h2>
    {#if s}
      <p class="meta" style="margin:0;font-size:14px">
        {t("sessions.detail.stopBody", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })}
      </p>
    {/if}
    <div class="d-foot">
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (stopOpen = false)}>{t("sessions.detail.keepRunning")}</button>
      <button class="btn danger" type="button" onclick={confirmStop}><Stop size={16} aria-hidden="true" />{t("sessions.detail.stopSession")}</button>
    </div>
  </div>
</Dialog>

<style>
  .shell-note {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin: 8px 0;
  }
  .exit-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .exit-row .btn {
    font-family: var(--font-ui);
  }
  /* The tabs scroll sideways on their own; the panel toggle stays at the right end of their row. */
  .views-bar {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }
  .views-bar > .tab-strip {
    flex: 1;
    min-width: 0;
  }
  /* Controls, not tabs, so they keep their full radius; the glass plate keeps the icon legible over the painting. */
  .bar-btn {
    align-self: center;
    flex-shrink: 0;
    --surface-2: var(--glass-2);
    background: var(--glass);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    box-shadow: var(--glass-rim);
    color: var(--ink);
  }
  .bar-btn:disabled {
    cursor: not-allowed;
    background: var(--glass);
    color: var(--ink-2);
  }
  @media (max-width: 720px) {
    .bar-btn {
      width: 44px;
      height: 44px;
    }
  }
  /* The shown tab's terminals share one dark panel, each placed by its share of the splits holding it. Placing them
     rather than nesting them keeps every terminal where it is in the page, so no move restarts one. */
  .panes {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .pane {
    position: absolute;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .pane.whole {
    inset: 0;
  }
  /* The 1 px line between split terminals, in the middle of a 9 px grip over both of them. term-dim at 60% keeps the
     line at 3:1 on term-bg in both themes, as a control's edge needs. */
  .pane-border {
    position: absolute;
    z-index: 2;
    width: 9px;
    margin-left: -4px;
    cursor: col-resize;
    touch-action: none;
  }
  .pane-border.across {
    height: 9px;
    margin-left: 0;
    margin-top: -4px;
    cursor: row-resize;
  }
  .pane-border::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 4px;
    width: 1px;
    background: color-mix(in srgb, var(--term-dim) 60%, transparent);
    transition: background-color 0.12s ease-out;
  }
  .pane-border.across::after {
    top: 4px;
    bottom: auto;
    left: 0;
    right: 0;
    width: auto;
    height: 1px;
  }
  .pane-border:hover::after,
  .pane-border:active::after {
    background: var(--term-dim);
  }
  .pane-border:focus-visible {
    outline: 2px solid var(--term-green);
    outline-offset: -2px;
    border-radius: 0;
  }
  /* While a border or a terminal is dragged, its cursor holds over everything it passes, and nothing gets selected. */
  :global(html[data-pane-drag]),
  :global(html[data-pane-drag] *) {
    user-select: none;
  }
  :global(html[data-pane-drag="col-resize"] *) {
    cursor: col-resize !important;
  }
  :global(html[data-pane-drag="row-resize"] *) {
    cursor: row-resize !important;
  }
  :global(html[data-pane-drag="grabbing"] *) {
    cursor: grabbing !important;
  }
  /* Where a dragged terminal would land: half of a terminal, lit in the cursor's green. */
  .drop-zone {
    position: absolute;
    z-index: 3;
    pointer-events: none;
    background: color-mix(in srgb, var(--term-green) 16%, transparent);
    border: 2px solid var(--term-green);
  }
  .drop-zone.left,
  .drop-zone.right {
    top: 0;
    bottom: 0;
    width: 50%;
  }
  .drop-zone.top,
  .drop-zone.bottom {
    left: 0;
    right: 0;
    height: 50%;
  }
  .drop-zone.left,
  .drop-zone.top {
    left: 0;
    top: 0;
  }
  .drop-zone.right {
    right: 0;
  }
  .drop-zone.bottom {
    bottom: 0;
  }
  /* Between tabs: a bar on the edge it would land at. On a tab: a dashed ring, to join it. */
  .tab {
    position: relative;
  }
  .tab.drop-before::before,
  .tab.drop-after::after {
    content: "";
    position: absolute;
    top: 4px;
    bottom: 0;
    width: 3px;
    border-radius: 2px;
    background: var(--forest);
  }
  .tab.drop-before::before {
    left: -4px;
  }
  .tab.drop-after::after {
    right: -4px;
  }
  .tab.drop-join {
    outline: 2px dashed var(--forest);
    outline-offset: -2px;
  }
  .tab.current.drop-join {
    outline-color: var(--term-green);
  }
  /* The dragged terminal's name follows the pointer, lifted above the page. */
  .drag-ghost {
    position: fixed;
    z-index: 60;
    max-width: 280px;
    padding: 6px 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
    transform: translate(12px, 12px);
    border-radius: var(--r-btn);
    background: var(--glass-pop);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    box-shadow: var(--glass-rim), var(--shadow-modal);
    color: var(--ink);
    font-size: 13px;
    font-weight: 600;
  }
  /* Names each terminal of a split tab. The one taking the keys is lit, underlined in the cursor's green. */
  .pane-head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    min-height: 36px;
    padding: 4px 4px 4px 14px;
    border-bottom: 1px solid #ede6c433;
    color: var(--term-dim);
  }
  .pane:focus-within > .pane-head {
    color: var(--term-text);
    box-shadow: inset 0 -2px 0 var(--term-green);
  }
  /* The terminal's name is its handle: drag it to move the terminal, or click it for the same moves as a menu. */
  .pane-name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    min-height: 28px;
    margin-left: -6px;
    padding: 2px 6px;
    border: 0;
    border-radius: var(--r-btn);
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: grab;
  }
  .pane-name:hover {
    background: #ffffff1a;
    color: var(--term-text);
  }
  .pane-name b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 600;
  }
  .pane-name small {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--term-dim);
  }
  .pane-name :global(svg) {
    flex-shrink: 0;
  }
  .pane-close {
    width: 28px;
    height: 28px;
    flex-shrink: 0;
    margin-left: auto;
    color: var(--term-dim);
  }
  .pane-close:hover {
    background: #ffffff1a;
    color: var(--term-text);
  }
  .pane-head :focus-visible {
    outline-color: var(--term-green);
  }
</style>
