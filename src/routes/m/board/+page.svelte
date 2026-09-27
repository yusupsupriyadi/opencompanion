<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Check from "phosphor-svelte/lib/Check";
  import Note from "phosphor-svelte/lib/Note";
  import Play from "phosphor-svelte/lib/Play";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import type { Column, Task } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import PhoneStatus from "$lib/PhoneStatus.svelte";
  import PhoneTopBar from "$lib/PhoneTopBar.svelte";
  import { CLI_LABEL, folderName, isLive, shortPath, waitingTitle } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { answer as sendAnswer, call, notify, onMessage, phone } from "$lib/phone.svelte";

  // The desktop Board, one column at a time (PRD FR-77). New cards and edits stay on the desktop.
  // The words are getters, so they follow the UI language.
  const COLS: { id: Column; name: string; empty: string }[] = [
    { id: "pending", get name() { return t("phone.board.pending"); }, get empty() { return t("phone.board.emptyPending"); } },
    { id: "todo", get name() { return t("phone.board.todo"); }, get empty() { return t("phone.board.emptyTodo"); } },
    { id: "progress", get name() { return t("phone.board.progress"); }, get empty() { return t("phone.board.emptyProgress"); } },
    { id: "done", get name() { return t("phone.board.done"); }, get empty() { return t("phone.board.emptyDone"); } },
  ];

  let tasks = $state<Task[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  const fromUrl = page.url.searchParams.get("col");
  let col = $state<Column>(COLS.find((c) => c.id === fromUrl)?.id ?? "todo");
  let openId = $state<string | null>(null);
  let sheetOpen = $state(false);
  let busy = $state(false);
  let failure = $state("");

  async function load(quiet = false) {
    if (!quiet) loadState = "loading";
    try {
      tasks = (await call<{ tasks: Task[] }>("/api/tasks")).tasks;
      loadState = "ready";
    } catch (e) {
      if (quiet) return;
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    load();
    return onMessage((m) => {
      if (m.type === "tasks") load(true);
    });
  });

  const byCol = $derived(
    Object.fromEntries(COLS.map((c) => [c.id, tasks.filter((task) => task.column === c.id).sort((a, b) => a.position - b.position)])) as Record<Column, Task[]>,
  );
  const current = $derived(COLS.find((c) => c.id === col) ?? COLS[1]);
  const opened = $derived(tasks.find((task) => task.id === openId) ?? null);
  const openedSession = $derived(opened ? sessionOf(opened) : undefined);

  // A card deleted on the computer while its sheet is open closes the sheet instead of emptying it.
  $effect(() => {
    if (sheetOpen && openId && !opened) {
      sheetOpen = false;
      notify(t("phone.board.cardDeleted"));
    }
  });

  function sessionOf(task: Task) {
    return task.sessionId ? phone.sessions.find((s) => s.id === task.sessionId) : undefined;
  }

  function pick(c: Column) {
    col = c;
    // Kept in the address, so Back from a session lands on the same column.
    goto(`/m/board?col=${c}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  function openCard(task: Task) {
    openId = task.id;
    failure = "";
    sheetOpen = true;
  }

  async function move(task: Task, to: Column) {
    busy = true;
    failure = "";
    try {
      await call(`/api/tasks/${task.id}/move`, { method: "POST", body: JSON.stringify({ column: to }) });
      await load(true);
      sheetOpen = false;
      notify(t("phone.board.moved", { column: String(COLS.find((c) => c.id === to)?.name), title: task.title }));
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function answer(sessionId: string, allow: boolean) {
    busy = true;
    failure = "";
    try {
      await sendAnswer(sessionId, allow);
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>{t("phone.nav.board")} · OpenCompanion</title></svelte:head>

<PhoneTopBar />
<main class="content dense" id="phone-board">
  <h1 class="m-h1">{t("phone.nav.board")}</h1>

  {#if loadState === "loading"}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("phone.board.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{t("phone.board.loadFailed", { error: loadError })}</p>
    <button class="btn secondary block" type="button" onclick={() => load()}>{t("phone.tryAgain")}</button>
  {:else}
    <div class="m-seg" role="group" aria-label={t("phone.board.columns")}>
      {#each COLS as c (c.id)}
        <button type="button" aria-pressed={col === c.id} onclick={() => pick(c.id)}>
          {c.name} <small>{byCol[c.id].length}</small>
        </button>
      {/each}
    </div>

    <section class="m-sec" aria-labelledby="board-col-title" id="board-column">
      <h2 id="board-col-title" class="sr-only">{current.name}</h2>
      {#if byCol[col].length === 0}
        <p class="m-p">{current.empty}</p>
      {/if}
      {#each byCol[col] as task (task.id)}
        {@const s = sessionOf(task)}
        <button class="m-card m-press" class:needs={s?.status === "waiting"} type="button" onclick={() => openCard(task)}>
          <span class="row">
            {#if task.cli}<CliMark kind={task.cli} small />{/if}
            <span class="task grow">{task.title}</span>
            {#if s}<PhoneStatus status={s.status} />{/if}
          </span>
          <span class="meta">
            {task.project ? folderName(task.project) : t("phone.board.noFolder")}{task.cli ? ` · ${CLI_LABEL[task.cli]}` : ""}
            {#if task.notes}<Note size={13} class="m-note" aria-hidden="true" /><span class="sr-only">{t("phone.board.hasNotes")}</span>{/if}
          </span>
        </button>
      {/each}
    </section>
  {/if}
</main>

<Dialog bind:open={sheetOpen} labelledby="card-sheet-title" sheet>
  {#if opened}
    {@const s = openedSession}
    <div class="d-body" style="gap:12px">
      <div class="m-sheet-head">
        <h2 id="card-sheet-title">{opened.title}</h2>
        <button class="m-icon-btn" type="button" aria-label={t("phone.board.close")} title={t("phone.board.close")} onclick={() => (sheetOpen = false)}><X size={20} aria-hidden="true" /></button>
      </div>
      <p class="meta" style="margin:0">
        {COLS.find((c) => c.id === opened.column)?.name}{opened.cli ? ` · ${CLI_LABEL[opened.cli]}` : ""}
        {#if opened.project} · <span class="mono" style="overflow-wrap:anywhere">{shortPath(opened.project)}</span>{/if}
      </p>
      {#if opened.notes}<p class="m-notes">{opened.notes}</p>{/if}

      {#if s?.status === "waiting"}
        <section class="m-needs" aria-labelledby="sheet-needs">
          <h3 id="sheet-needs" style="margin:0;font-size:15px;font-weight:800">{waitingTitle(s)}</h3>
          {#if s.waiting?.detail}<div class="cmd"><small>{s.waiting.tool ?? t("phone.request")}</small><code>{s.waiting.detail}</code></div>{/if}
          {#if s.waiting?.canAnswer}
            <div class="pair-btns">
              <button class="btn accent" type="button" disabled={busy} onclick={() => answer(s.id, true)}><Check size={16} aria-hidden="true" />{t("phone.approve")}</button>
              <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(s.id, false)}><X size={16} aria-hidden="true" />{t("phone.deny")}</button>
            </div>
          {:else}
            <p class="small" style="margin:0">{t("phone.board.openToAnswer")}</p>
          {/if}
        </section>
      {/if}

      {#if failure}<p class="err-text m-appear" role="alert" style="margin:0">{tb(failure)}</p>{/if}

      <div class="sheet-acts">
        {#if (opened.column === "todo" || opened.column === "pending") && !(s && isLive(s))}
          <a class="btn primary" href="/m/new?task={opened.id}"><Play size={16} aria-hidden="true" />{opened.project ? t("phone.runIn", { folder: folderName(opened.project) }) : t("phone.board.runCard")}</a>
        {/if}
        {#if opened.sessionId}
          <a class="btn secondary" href="/m/session?id={opened.sessionId}"><TerminalWindow size={16} aria-hidden="true" />{t("phone.openSession")}</a>
        {/if}
      </div>

      <div class="field">
        <span class="label" id="move-label">{t("phone.board.moveTo")}</span>
        <div class="move-row" role="group" aria-labelledby="move-label">
          {#each COLS.filter((c) => c.id !== opened.column) as c (c.id)}
            <button class="btn secondary" type="button" disabled={busy} onclick={() => move(opened, c.id)}>{c.name}</button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .m-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-card :global(.m-note) {
    margin-left: 6px;
    vertical-align: -2px;
  }
  /* The title leaves room for Close, the sheet's one way out besides Escape. */
  .m-sheet-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .m-sheet-head h2 {
    flex: 1;
    min-width: 0;
    padding-top: 6px;
    font-size: 18px;
    overflow-wrap: anywhere;
  }
  .m-sheet-head .m-icon-btn {
    margin: -2px -8px 0 0;
  }
</style>
