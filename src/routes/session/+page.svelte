<script lang="ts">
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
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
  import { MAX_PANES, PaneLayout, TERM, type PaneTab } from "$lib/panes.svelte";
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
  let newShellOpen = $state(false);
  // The tab a New terminal dialog opened from Split puts its shell in.
  let splitInto = $state<string | null>(null);
  let restarting = $state("");
  let shellFailure = $state("");

  const paneName = (p: string) => (p === TERM ? (s ? CLI_LABEL[s.cli] : "") : (shells.names.get(p) ?? ""));
  const panelId = (p: string) => (p === TERM ? "session-term" : `shell-panel-${p}`);
  const tabButtonId = (p: string) => (p === TERM ? "view-tab-term" : `shell-tab-${p}`);
  const grow = (p: string) => (shown ? (shown.sizes[shown.panes.indexOf(p)] ?? 1) : 1);

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

  // The border between two split terminals moves with the pointer, or with the arrow keys along it (Home and End go
  // as far as they can). Neither terminal gets narrower, or lower, than MIN_PANE.
  const MIN_PANE = 120;
  const STEP = 0.05;
  let panesBox: HTMLDivElement | undefined = $state();

  function boxLength(tab: PaneTab) {
    const r = panesBox?.getBoundingClientRect();
    return r ? (tab.dir === "row" ? r.width : r.height) : 0;
  }

  function borderKeys(e: KeyboardEvent, i: number) {
    if (!shown) return;
    const row = shown.dir === "row";
    const size = shown.sizes[i];
    const keys: Record<string, number> = {
      [row ? "ArrowLeft" : "ArrowUp"]: size - STEP,
      [row ? "ArrowRight" : "ArrowDown"]: size + STEP,
      Home: 0,
      End: 1,
    };
    if (!(e.key in keys)) return;
    e.preventDefault();
    const length = boxLength(shown);
    layout.resize(shownTab, i, keys[e.key], length > 0 ? MIN_PANE / length : 0.1);
  }

  function borderDrag(e: PointerEvent, i: number) {
    if (!shown || e.button !== 0) return;
    const tab = shown;
    const lead = shownTab;
    const length = boxLength(tab);
    if (length <= 0) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    const at = (p: PointerEvent) => (tab.dir === "row" ? p.clientX : p.clientY);
    const from = at(e);
    const start = tab.sizes[i];
    const move = (m: PointerEvent) => layout.resize(lead, i, start + (at(m) - from) / length, MIN_PANE / length);
    const end = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("lostpointercapture", end);
    };
    el.setPointerCapture(e.pointerId);
    el.addEventListener("pointermove", move);
    el.addEventListener("lostpointercapture", end);
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
              <div class="tab" class:current={shownTab === lead}>
                <button
                  class="tab-pick"
                  type="button"
                  id={tabButtonId(lead)}
                  aria-current={shownTab === lead ? "true" : undefined}
                  aria-controls={tab.panes.map(panelId).join(" ")}
                  title={more ? tab.panes.map(paneName).join(", ") : undefined}
                  onclick={() => (mainTab = lead)}
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
              title={shown.panes.length >= MAX_PANES ? t("terminal.splitFull", { n: MAX_PANES }) : shown.dir === "column" ? t("terminal.splitBelow") : t("terminal.splitBeside")}
              disabled={!canSplit}
              onclick={() => newShell(shownTab)}
            >
              {#if shown.dir === "column"}<SquareSplitVertical size={20} aria-hidden="true" />{:else}<SquareSplitHorizontal size={20} aria-hidden="true" />{/if}
            </button>
            {#if split}
              <button
                class="icon-btn bar-btn"
                type="button"
                id="btn-stack-terminals"
                aria-label={t("terminal.stack")}
                aria-pressed={shown.dir === "column"}
                title={shown.dir === "column" ? t("terminal.sideHint") : t("terminal.stackHint")}
                onclick={() => layout.flip(shownTab)}
              >
                <Rows size={20} weight={shown.dir === "column" ? "fill" : "regular"} aria-hidden="true" />
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
        <!-- Every terminal stays mounted in one dark panel; the shown tab's ones are visible, in the order they were opened. -->
        {#snippet paneTop(p: string, x: TerminalInfo | null)}
          {@const i = shown ? shown.panes.indexOf(p) : -1}
          {#if shown && i > 0}
            <!-- A focusable separator is ARIA's window splitter, a widget; Svelte's check counts every separator as static. -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
            <div
              class="pane-border"
              role="separator"
              tabindex="0"
              aria-orientation={shown.dir === "row" ? "vertical" : "horizontal"}
              aria-controls={panelId(shown.panes[i - 1])}
              aria-label={t("terminal.resize", { first: paneName(shown.panes[i - 1]), second: paneName(p) })}
              aria-valuenow={Math.round((shown.sizes[i - 1] / (shown.sizes[i - 1] + shown.sizes[i])) * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              onkeydown={(e) => borderKeys(e, i - 1)}
              onpointerdown={(e) => borderDrag(e, i - 1)}
            ></div>
          {/if}
          {#if inSplit.has(p)}
            <div class="pane-head">
              <b>{paneName(p)}</b>
              {#if !x}
                <small>{s?.mode === "interactive" ? t("workspace.viewer.terminal") : t("workspace.viewer.output")}</small>
              {:else if !x.running}
                <span class="chip idle">{t("terminal.exited")}</span>
              {/if}
              {#if x}
                <button class="icon-btn pane-close" type="button" aria-label={t("terminal.closeNamed", { name: paneName(p) })} title={t("terminal.closeHint", { shell: x.shellLabel })} onclick={() => closeShell(x)}>
                  <X size={14} aria-hidden="true" />
                </button>
              {/if}
            </div>
          {/if}
        {/snippet}
        <div class="term joined" id="session-panes">
          <div class="panes" class:stacked={shown?.dir === "column"} bind:this={panesBox} onfocusin={notePane}>
            <section class="pane" id="session-term" data-pane={TERM} hidden={!shown?.panes.includes(TERM)} style:flex-grow={grow(TERM)} aria-label={t("sessions.detail.output")}>
              {@render paneTop(TERM, null)}
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
            {#each shells.list as x (x.id)}
              {@const label = t("terminal.label", { shell: shells.names.get(x.id) ?? x.shellLabel, folder: folderName(s.cwd) })}
              <section class="pane" id="shell-panel-{x.id}" data-pane={x.id} aria-label={label} hidden={!shown?.panes.includes(x.id)} style:flex-grow={grow(x.id)}>
                {@render paneTop(x.id, x)}
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
            {/each}
            {#if viewer}
              <section class="pane" id="session-viewer" hidden={shownTab !== "viewer"}>
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
  /* The shown tab's terminals share one dark panel, side by side or stacked, each by its share of the tab. */
  .panes {
    flex: 1;
    display: flex;
    min-width: 0;
    min-height: 0;
  }
  .panes.stacked {
    flex-direction: column;
  }
  .pane {
    position: relative;
    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* The line before a split terminal, with a 7 px grip over that terminal's padding. term-dim at 60% keeps the line
     at 3:1 on term-bg in both themes, as a control's edge needs. */
  .pane-border {
    position: absolute;
    z-index: 1;
    top: 0;
    bottom: 0;
    left: 0;
    width: 7px;
    border-left: 1px solid color-mix(in srgb, var(--term-dim) 60%, transparent);
    cursor: col-resize;
    touch-action: none;
    transition: border-color 0.12s ease-out;
  }
  .stacked .pane-border {
    right: 0;
    bottom: auto;
    width: auto;
    height: 7px;
    border-left: 0;
    border-top: 1px solid color-mix(in srgb, var(--term-dim) 60%, transparent);
    cursor: row-resize;
  }
  .pane-border:hover,
  .pane-border:active {
    border-color: var(--term-dim);
  }
  .pane-border:focus-visible {
    outline: 2px solid var(--term-green);
    outline-offset: -2px;
    border-radius: 0;
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
  .pane-head b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 600;
  }
  .pane-head small {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--term-dim);
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
