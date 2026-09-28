<script lang="ts" module>
  // The tab shown last, so coming back to this screen opens the same one.
  let lastShown = "";
</script>

<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import Plus from "phosphor-svelte/lib/Plus";
  import X from "phosphor-svelte/lib/X";
  import { onMount, tick } from "svelte";
  import { api, errorText, type TerminalInfo } from "$lib/api";
  import Dialog from "$lib/Dialog.svelte";
  import { folderName } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { pasteKey } from "$lib/platform";
  import Terminal from "$lib/Terminal.svelte";
  import TerminalForm from "$lib/TerminalForm.svelte";

  let terms = $state<TerminalInfo[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let shown = $state("");
  let newOpen = $state(false);
  let restarting = $state("");
  let failure = $state("");

  const current = $derived(terms.find((x) => x.id === shown) ?? terms[0]);

  // Terminals in folders with the same name are numbered in the order they were opened.
  const names = $derived.by(() => {
    const seen = new Map<string, number>();
    return new Map(
      terms.map((x) => {
        const base = folderName(x.cwd);
        const n = (seen.get(base) ?? 0) + 1;
        seen.set(base, n);
        return [x.id, n === 1 ? base : `${base} ${n}`];
      }),
    );
  });

  // Closed here, so a late event cannot bring the tab back.
  const closed = new Set<string>();
  // Events that arrive while the list loads, applied on top of it.
  let early: TerminalInfo[] | null = [];

  /** `keep` leaves a terminal already listed as it is: an event may have been newer than this copy. */
  function upsert(info: TerminalInfo, keep = false) {
    if (closed.has(info.id)) return;
    const i = terms.findIndex((x) => x.id === info.id);
    if (i < 0) terms.push(info);
    else if (!keep) terms[i] = info;
  }

  function show(id: string) {
    shown = id;
    lastShown = id;
  }

  async function load() {
    loadState = "loading";
    early = [];
    try {
      terms = await api.terminalList();
      early.splice(0).forEach((info) => upsert(info));
      early = null;
      show(terms.some((x) => x.id === lastShown) ? lastShown : (terms[0]?.id ?? ""));
      loadState = "ready";
    } catch (e) {
      loadError = errorText(e);
      loadState = "error";
    }
  }

  onMount(() => {
    const un = listen<TerminalInfo>("terminal-changed", (e) => {
      if (early) early.push(e.payload);
      else upsert(e.payload);
    });
    un.then(load);
    return () => {
      un.then((f) => f());
    };
  });

  async function open(cwd: string, shell: string) {
    const info = await api.terminalOpen(cwd, shell);
    upsert(info, true);
    show(info.id);
    newOpen = false;
  }

  async function close(x: TerminalInfo) {
    failure = "";
    const at = terms.findIndex((y) => y.id === x.id);
    try {
      await api.terminalClose(x.id);
    } catch (e) {
      failure = errorText(e);
      return;
    }
    closed.add(x.id);
    terms = terms.filter((y) => y.id !== x.id);
    const next = terms[Math.min(at, terms.length - 1)];
    if (shown === x.id) show(next?.id ?? "");
    // The button that closed it is gone: focus goes to the tab shown now, or to New terminal.
    await tick();
    const target = next ? document.getElementById(`term-tab-${next.id}`) : document.getElementById("btn-new-terminal");
    target?.focus();
  }

  async function restart(x: TerminalInfo) {
    failure = "";
    restarting = x.id;
    try {
      upsert(await api.terminalRestart(x.id));
    } catch (e) {
      failure = errorText(e);
    } finally {
      restarting = "";
    }
  }
</script>

<svelte:head><title>{t("terminal.heading")} · OpenCompanion</title></svelte:head>

<main class="main term-page" id="terminal-main">
  <h1 class="sr-only">{t("terminal.heading")}</h1>
  {#if loadState === "loading"}
    <p class="hint" role="status">{t("terminal.loading")}</p>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("terminal.loadFailed")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={load}>{t("sessions.tryAgain")}</button>
    </div>
  {:else if terms.length === 0}
    <div class="state-box" id="terminal-empty">
      <h2>{t("terminal.emptyTitle")}</h2>
      <p>{t("terminal.emptyBody")}</p>
      <button class="btn primary" type="button" id="btn-new-terminal" onclick={() => (newOpen = true)}>
        <Plus size={16} aria-hidden="true" />{t("terminal.new")}
      </button>
    </div>
  {:else}
    <div class="tab-strip" id="terminal-tabs" role="group" aria-label={t("terminal.tabs")}>
      {#each terms as x (x.id)}
        {@const name = names.get(x.id) ?? folderName(x.cwd)}
        <div class="tab" class:current={x.id === current?.id}>
          <button
            class="tab-pick"
            type="button"
            id="term-tab-{x.id}"
            aria-current={x.id === current?.id ? "true" : undefined}
            aria-controls="term-panel-{x.id}"
            title={t("terminal.tabTitle", { path: x.cwd, shell: x.shellLabel })}
            onclick={() => show(x.id)}
          >
            <b class="mono">{name}</b>
            <small>{x.shellLabel}</small>
            {#if !x.running}<span class="chip idle">{t("terminal.exited")}</span>{/if}
          </button>
          <button
            class="icon-btn tab-close"
            type="button"
            aria-label={t("terminal.closeNamed", { name })}
            title={t("terminal.closeHint", { shell: x.shellLabel })}
            onclick={() => close(x)}
          >
            <X size={14} aria-hidden="true" />
          </button>
        </div>
      {/each}
      <button class="icon-btn tab-new" type="button" id="btn-new-terminal" aria-label={t("terminal.new")} title={t("terminal.new")} onclick={() => (newOpen = true)}>
        <Plus size={16} aria-hidden="true" />
      </button>
    </div>
    {#if failure}<p class="err-text" role="alert">{tb(failure)}</p>{/if}

    {#each terms as x (x.id)}
      {@const name = names.get(x.id) ?? folderName(x.cwd)}
      <section class="term pane" id="term-panel-{x.id}" aria-label={t("terminal.label", { shell: x.shellLabel, folder: name })} hidden={x.id !== current?.id}>
        {#key `${x.id}-${x.startedAt}`}
          <Terminal kind="terminal" id={x.id} live={x.running} label={t("terminal.label", { shell: x.shellLabel, folder: name })} />
        {/key}
        {#if x.running}
          <div class="term-note">{t("terminal.live", { paste: pasteKey() })}</div>
        {:else}
          <div class="term-note exit-row">
            <span class="grow">
              {x.exitCode === null ? t("terminal.exitedNoCode", { shell: x.shellLabel }) : t("terminal.exitedCode", { shell: x.shellLabel, code: x.exitCode })}
            </span>
            <button class="btn primary sm" type="button" disabled={restarting === x.id} onclick={() => restart(x)}>
              <ArrowClockwise size={14} aria-hidden="true" />{restarting === x.id ? t("terminal.restarting") : t("terminal.restart")}
            </button>
            <button class="btn secondary sm" type="button" onclick={() => close(x)}>
              <X size={14} aria-hidden="true" />{t("shell.close")}
            </button>
          </div>
        {/if}
      </section>
    {/each}
  {/if}
</main>

<Dialog bind:open={newOpen} labelledby="nt-title">
  <TerminalForm initialCwd={current?.cwd ?? ""} onsubmit={open} oncancel={() => (newOpen = false)} />
</Dialog>

<style>
  .main.term-page {
    gap: 0;
    padding: 24px 40px 28px;
  }
  .tab-strip {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    overflow-x: auto;
    flex-shrink: 0;
  }
  /* A tab is a plate over the painting; the shown one takes the terminal's own colors and
     joins the panel below, so it reads as the lid of that terminal. */
  .tab {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    max-width: 260px;
    padding-right: 4px;
    border-radius: var(--r-btn) var(--r-btn) 0 0;
    --surface: var(--glass);
    --surface-2: var(--glass-2);
    background: var(--surface);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    box-shadow: var(--glass-rim);
  }
  .tab.current {
    background: var(--term-bg);
    color: var(--term-text);
    box-shadow: none;
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
  }
  .tab-pick {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    min-height: 38px;
    padding: 6px 6px 6px 12px;
    border: 0;
    background: transparent;
    cursor: pointer;
  }
  .tab-pick b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    font-weight: 600;
  }
  .tab-pick small {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--ink-2);
  }
  .tab.current .tab-pick small {
    color: var(--term-dim);
  }
  .tab-close {
    width: 28px;
    height: 28px;
    flex-shrink: 0;
  }
  .tab.current .tab-close {
    color: var(--term-dim);
  }
  .tab.current .tab-close:hover {
    background: #ffffff1a;
    color: var(--term-text);
  }
  .tab.current :focus-visible {
    outline-color: var(--term-green);
  }
  .tab-new {
    flex-shrink: 0;
    margin-left: 2px;
    align-self: center;
  }
  .err-text {
    margin: 8px 0 0;
  }
  .pane {
    flex: 1;
    min-height: 320px;
    border-top-left-radius: 0;
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
</style>
