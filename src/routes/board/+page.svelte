<script lang="ts">
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import HandPalm from "phosphor-svelte/lib/HandPalm";
  import Note from "phosphor-svelte/lib/Note";
  import Play from "phosphor-svelte/lib/Play";
  import Plus from "phosphor-svelte/lib/Plus";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Trash from "phosphor-svelte/lib/Trash";
  import WarningCircle from "phosphor-svelte/lib/WarningCircle";
  import X from "phosphor-svelte/lib/X";
  import { onMount, tick } from "svelte";
  import { api, errorText, type CliKind, type Column, type Task } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import FolderField from "$lib/FolderField.svelte";
  import SessionForm, { type SessionValues } from "$lib/SessionForm.svelte";
  import { CLI_LABEL, duration, folderName } from "$lib/format";
  import { app, showToast } from "$lib/store.svelte";

  const COLS: { id: Column; name: string; hint: string; empty: string }[] = [
    { id: "pending", name: "Pending", hint: "Not ready yet", empty: "Nothing pending. New cards start here." },
    { id: "todo", name: "Todo", hint: "Ready to hand to a CLI", empty: "No ready tasks. Move a pending card here once it is clear enough to run." },
    { id: "progress", name: "In progress", hint: "A CLI session is on it", empty: "Nothing in progress. Press Run on a Todo card." },
    { id: "done", name: "Done", hint: "Finished", empty: "Finished cards land here." },
  ];

  let tasks = $state<Task[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let filter = $state("");
  let dragId = $state<string | null>(null);
  let dropCol = $state<Column | null>(null);

  // Card dialog
  let cardOpen = $state(false);
  let editing = $state<Task | null>(null);
  let form = $state({ title: "", notes: "", project: "", cli: "" as CliKind | "", column: "pending" as Column });
  let titleError = $state("");
  let formError = $state("");
  let confirmDelete = $state(false);
  let saving = $state(false);

  // Run dialog
  let runOpen = $state(false);
  let runTask = $state<Task | null>(null);

  async function load() {
    try {
      tasks = await api.listTasks();
      loadState = "ready";
    } catch (e) {
      loadState = "error";
      loadError = errorText(e);
    }
  }

  onMount(() => {
    load();
    const un = listen("tasks-changed", () => load());
    return () => {
      un.then((f) => f());
    };
  });

  const projects = $derived([...new Set(tasks.map((t) => t.project).filter(Boolean))].sort());
  const visible = $derived(filter ? tasks.filter((t) => t.project === filter) : tasks);
  const byCol = $derived(
    Object.fromEntries(COLS.map((c) => [c.id, visible.filter((t) => t.column === c.id).sort((a, b) => a.position - b.position)])) as Record<Column, Task[]>,
  );

  function sessionOf(t: Task) {
    return t.sessionId ? app.sessions.find((s) => s.id === t.sessionId) : undefined;
  }

  function openNew() {
    editing = null;
    form = { title: "", notes: "", project: filter, cli: "", column: "pending" };
    titleError = "";
    formError = "";
    confirmDelete = false;
    cardOpen = true;
  }

  function openEdit(t: Task) {
    editing = t;
    form = { title: t.title, notes: t.notes, project: t.project, cli: t.cli ?? "", column: t.column };
    titleError = "";
    formError = "";
    confirmDelete = false;
    cardOpen = true;
  }

  async function focusCard(id: string) {
    await tick();
    document.querySelector<HTMLButtonElement>(`[data-card="${id}"] .tc-title`)?.focus();
  }

  async function saveCard(e: SubmitEvent) {
    e.preventDefault();
    if (!form.title.trim()) {
      titleError = "Give the card a title.";
      document.getElementById("cd-title")?.focus();
      return;
    }
    saving = true;
    formError = "";
    try {
      const moved = editing && editing.column !== form.column;
      const saved = await api.saveTask({ id: editing?.id, title: form.title, notes: form.notes, project: form.project, cli: form.cli || null, column: form.column });
      cardOpen = false;
      await load();
      showToast(moved ? `Moved "${saved.title}" to ${COLS.find((c) => c.id === saved.column)?.name}.` : editing ? "Card saved." : `Added "${saved.title}" to ${COLS.find((c) => c.id === saved.column)?.name}.`);
      setTimeout(() => focusCard(saved.id), 0);
    } catch (err) {
      formError = errorText(err);
    } finally {
      saving = false;
    }
  }

  async function deleteCard() {
    if (!editing) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    try {
      await api.deleteTask(editing.id);
      cardOpen = false;
      await load();
      showToast(`Deleted "${editing.title}".`);
    } catch (err) {
      formError = errorText(err);
    }
  }

  function openRun(t: Task) {
    runTask = t;
    runOpen = true;
  }

  async function startRun(v: SessionValues) {
    if (!runTask) return;
    const s = await api.runTask({ id: runTask.id, ...v });
    runOpen = false;
    await load();
    showToast(`Started ${CLI_LABEL[v.cli]} in ${folderName(v.cwd)}. The card moved to In progress.`);
    await goto(`/session?id=${s.id}`);
  }

  function ondragstart(e: DragEvent, t: Task) {
    dragId = t.id;
    e.dataTransfer?.setData("text/plain", t.id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  async function ondrop(e: DragEvent, col: Column) {
    e.preventDefault();
    const id = dragId ?? e.dataTransfer?.getData("text/plain");
    dropCol = null;
    dragId = null;
    if (!id) return;
    const over = (e.target as HTMLElement).closest<HTMLElement>("[data-card]")?.dataset.card;
    const task = tasks.find((t) => t.id === id);
    if (!task || (task.column === col && (!over || over === id))) return;
    try {
      await api.moveTask(id, col, over && over !== id ? over : null);
      await load();
      showToast(`Moved "${task.title}" to ${COLS.find((c) => c.id === col)?.name}.`);
    } catch (err) {
      showToast(errorText(err));
    }
  }
</script>

<svelte:head><title>Board · OpenCompanion</title></svelte:head>

<main class="main board-main" id="board-main">
  <header class="page-head">
    <div class="grow">
      <h1>Board</h1>
      <p class="sub">Tasks for your coding CLIs. Press Run on a Todo card to start a session; the card follows the session and moves to Done when it finishes.</p>
    </div>
    <div class="board-tools">
      <label class="sr-only" for="bd-filter">Filter by project</label>
      <select class="select" id="bd-filter" bind:value={filter}>
        <option value="">All projects</option>
        {#each projects as p (p)}<option value={p}>{folderName(p)}</option>{/each}
      </select>
      <button class="btn primary" type="button" id="btn-new-card" onclick={openNew}><Plus size={16} aria-hidden="true" />New card</button>
    </div>
  </header>

  {#if loadState === "loading"}
    <p class="hint" role="status">Loading the board…</p>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>The board could not be loaded</h2>
      <p>{loadError}</p>
      <button class="btn secondary" type="button" onclick={load}>Try again</button>
    </div>
  {:else}
    <div class="board" id="board">
      {#each COLS as col (col.id)}
        <section class="col" aria-labelledby="h-{col.id}">
          <div class="col-head">
            <div class="grow">
              <h2 id="h-{col.id}">{col.name} <span class="meta" style="font-weight:600">{byCol[col.id].length}</span></h2>
              <p class="hint-line">{col.hint}</p>
            </div>
          </div>
          <div
            class="col-body"
            class:drop={dropCol === col.id}
            role="list"
            aria-label="{col.name} cards"
            ondragover={(e) => {
              e.preventDefault();
              dropCol = col.id;
            }}
            ondragleave={(e) => {
              if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) dropCol = null;
            }}
            ondrop={(e) => ondrop(e, col.id)}
          >
            {#if byCol[col.id].length === 0}
              <p class="col-empty">{col.empty}</p>
            {/if}
            {#each byCol[col.id] as t (t.id)}
              {@const s = sessionOf(t)}
              <div
                class="tcard"
                class:needs={s?.status === "waiting"}
                class:dragging={dragId === t.id}
                data-card={t.id}
                role="listitem"
                draggable="true"
                ondragstart={(e) => ondragstart(e, t)}
                ondragend={() => {
                  dragId = null;
                  dropCol = null;
                }}
              >
                <div class="tc-top">
                  {#if t.cli}<CliMark kind={t.cli} small />{/if}
                  <button class="tc-title" type="button" onclick={() => openEdit(t)} title="Edit {t.title}">{t.title}</button>
                  {#if t.column === "todo"}
                    <button class="tc-act run-act" type="button" aria-label="Run in {t.project ? folderName(t.project) : 'a folder'}" title="Run" onclick={() => openRun(t)}>
                      <Play size={16} aria-hidden="true" />
                    </button>
                  {:else if t.sessionId}
                    <a class="tc-act" href="/session?id={t.sessionId}" aria-label="Open session" title="Open session"><ArrowSquareOut size={16} aria-hidden="true" /></a>
                  {/if}
                </div>
                <div class="tc-meta">
                  {#if t.project}
                    <span title={t.project}><FolderSimple size={14} aria-hidden="true" /><span class="mono">{folderName(t.project)}</span></span>
                  {/if}
                  {#if t.column === "progress" && s}
                    {#if s.status === "waiting"}
                      <span class="st-i-wait" title="Waiting for you"><HandPalm size={14} aria-hidden="true" /><span class="sr-only">Waiting for you</span>{duration(app.now - (s.waiting?.since ?? s.updatedAt))}</span>
                    {:else if s.status === "error"}
                      <span class="st-i-err" title="Error"><WarningCircle size={14} aria-hidden="true" /><span>Error</span></span>
                    {:else if s.status === "stopped"}
                      <span title="Stopped"><X size={14} aria-hidden="true" /><span>Stopped</span></span>
                    {:else}
                      <span class="st-i-run" title="Running"><SpinnerGap size={14} aria-hidden="true" /><span class="sr-only">Running</span>{duration(app.now - s.startedAt)}</span>
                    {/if}
                  {:else if t.column === "done" && s}
                    <span class="st-i-done" title="Finished"><CheckCircle size={14} aria-hidden="true" /><span class="sr-only">Finished, took</span>{duration((s.endedAt ?? s.updatedAt) - s.startedAt)}</span>
                  {:else if t.notes}
                    <span title="Has notes"><Note size={14} aria-hidden="true" /><span class="sr-only">Has notes</span></span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
    <p class="meta note" style="margin:0">Drag cards between columns, or open a card and change its column.</p>
  {/if}
</main>

<Dialog bind:open={cardOpen} labelledby="cd-heading">
  <form class="d-body" novalidate onsubmit={saveCard}>
    <div class="row">
      <h2 id="cd-heading" class="grow">{editing ? "Edit card" : "New card"}</h2>
      <button class="icon-btn" type="button" aria-label="Close" onclick={() => (cardOpen = false)}><X size={18} aria-hidden="true" /></button>
    </div>
    <div class="field">
      <label class="label" for="cd-title">Title</label>
      <input class="input" id="cd-title" bind:value={form.title} oninput={() => (titleError = "")} placeholder="What should get done?" aria-invalid={titleError ? "true" : undefined} aria-describedby={titleError ? "cd-title-error" : undefined} autocomplete="off" />
      {#if titleError}<p class="error" id="cd-title-error">{titleError}</p>{/if}
    </div>
    <div class="field">
      <label class="label" for="cd-notes">Notes</label>
      <textarea class="textarea" id="cd-notes" bind:value={form.notes} placeholder="Details, acceptance criteria, or open questions. Used as the prompt when you press Run."></textarea>
    </div>
    <FolderField id="cd-project" label="Project" bind:value={form.project} />
    <div class="opts">
      <div class="field">
        <label class="label" for="cd-cli">CLI</label>
        <select class="select" id="cd-cli" bind:value={form.cli}>
          <option value="">Decide later</option>
          {#each app.clis as c (c.kind)}<option value={c.kind} disabled={!c.path}>{c.label}{c.path ? "" : " (not installed)"}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label class="label" for="cd-col">Column</label>
        <select class="select" id="cd-col" bind:value={form.column}>
          {#each COLS as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
        </select>
      </div>
    </div>
    {#if formError}<p class="err-text" role="alert" style="margin:0">{formError}</p>{/if}
    <div class="d-foot">
      {#if editing}
        <button class="btn ghost danger-text" type="button" onclick={deleteCard}>
          <Trash size={16} aria-hidden="true" /><span>{confirmDelete ? "Press again to delete" : "Delete"}</span>
        </button>
      {/if}
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (cardOpen = false)}>Cancel</button>
      <button class="btn primary" type="submit" disabled={saving}>Save card</button>
    </div>
  </form>
</Dialog>

<Dialog bind:open={runOpen} labelledby="rd-title">
  {#if runTask}
    <SessionForm
      idPrefix="rd"
      title="Run this card"
      initial={{ cli: runTask.cli ?? undefined, cwd: runTask.project, mode: "interactive", prompt: runTask.notes || runTask.title }}
      onsubmit={startRun}
      oncancel={() => (runOpen = false)}
    />
  {/if}
</Dialog>

<style>
  .main.board-main {
    gap: 20px;
    overflow: hidden;
  }
  .board-tools {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .board-tools .select {
    width: auto;
    min-width: 180px;
    padding: 8px 10px;
  }
  .board {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(4, minmax(240px, 1fr));
    gap: 14px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .col-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px;
  }
  .col-head h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 800;
  }
  .hint-line {
    font-size: 12px;
    color: var(--ink-2);
    margin: 2px 0 0;
  }
  .col-body {
    flex: 1;
    min-height: 140px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
    border-radius: var(--r-md);
    background: var(--surface-2);
  }
  .col-body.drop {
    outline: 2px dashed var(--forest);
    outline-offset: -2px;
  }
  .col-empty {
    margin: 0;
    padding: 8px 4px;
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.45;
  }
  .tcard {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line);
    cursor: grab;
  }
  .tcard.needs {
    border: 1.5px solid var(--accent);
    padding: 9.5px;
  }
  .tcard.dragging {
    opacity: 0.45;
  }
  .tc-top {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .tc-title {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    text-align: left;
    font: 700 14px/1.35 var(--font-ui);
    color: var(--ink);
    cursor: pointer;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .tc-title:hover {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .tc-act {
    flex: none;
    width: 28px;
    height: 28px;
    margin: -4px -4px 0 0;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--ink-2);
    cursor: pointer;
  }
  .tc-act:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .tc-act.run-act {
    color: var(--forest);
  }
  .tc-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 12px;
    color: var(--ink-2);
    min-width: 0;
  }
  .tc-meta > span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .tc-meta .mono {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st-i-run,
  .st-i-done {
    color: var(--forest);
  }
  .st-i-wait {
    color: var(--accent);
  }
  .st-i-err {
    color: var(--st-err);
  }
  .danger-text {
    color: var(--st-err);
  }
  @media (max-width: 1100px) {
    .board {
      grid-template-columns: repeat(4, 260px);
    }
  }
  @media (max-width: 720px) {
    .tc-act {
      width: 40px;
      height: 40px;
    }
    .main.board-main {
      overflow: visible;
    }
    .board {
      grid-template-columns: 1fr;
      overflow: visible;
    }
    .col-body {
      overflow: visible;
    }
    .board-tools {
      flex-wrap: wrap;
    }
  }
</style>
