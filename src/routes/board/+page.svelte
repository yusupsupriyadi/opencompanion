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
  import { t, tb, type Key } from "$lib/i18n.svelte";
  import { app, showToast } from "$lib/store.svelte";

  // Keys, not text: t() runs where it is read, so the columns follow a language change.
  const COLS: { id: Column; name: Key; hint: Key; empty: Key }[] = [
    { id: "pending", name: "work.board.colPending", hint: "work.board.colPendingHint", empty: "work.board.colPendingEmpty" },
    { id: "todo", name: "work.board.colTodo", hint: "work.board.colTodoHint", empty: "work.board.colTodoEmpty" },
    { id: "progress", name: "work.board.colProgress", hint: "work.board.colProgressHint", empty: "work.board.colProgressEmpty" },
    { id: "done", name: "work.board.colDone", hint: "work.board.colDoneHint", empty: "work.board.colDoneEmpty" },
  ];

  function colName(id: Column): string {
    const col = COLS.find((c) => c.id === id);
    return col ? t(col.name) : id;
  }

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
      titleError = t("work.board.titleRequired");
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
      // The title goes last: t() fills vars in order, so a title holding "{column}" stays as typed.
      const vars = { column: colName(saved.column), title: saved.title };
      showToast(moved ? t("work.board.movedToast", vars) : editing ? t("work.board.savedToast") : t("work.board.addedToast", vars));
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
      showToast(t("work.board.deletedToast", { title: editing.title }));
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
    showToast(t("work.board.startedToast", { cli: CLI_LABEL[v.cli], folder: folderName(v.cwd) }));
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
      showToast(t("work.board.movedToast", { column: colName(col), title: task.title }));
    } catch (err) {
      showToast(tb(errorText(err)));
    }
  }
</script>

<svelte:head><title>{t("work.board.pageTitle")}</title></svelte:head>

<main class="main board-main" id="board-main">
  <header class="page-head">
    <div class="grow">
      <h1>{t("work.board.heading")}</h1>
      <p class="sub">{t("work.board.sub")}</p>
    </div>
    <div class="board-tools">
      <label class="sr-only" for="bd-filter">{t("work.board.filterLabel")}</label>
      <select class="select" id="bd-filter" bind:value={filter}>
        <option value="">{t("work.board.allProjects")}</option>
        {#each projects as p (p)}<option value={p}>{folderName(p)}</option>{/each}
      </select>
      <button class="btn primary" type="button" id="btn-new-card" onclick={openNew}><Plus size={16} aria-hidden="true" />{t("work.board.newCard")}</button>
    </div>
  </header>

  {#if loadState === "loading"}
    <p class="hint" role="status">{t("work.board.loading")}</p>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("work.board.loadError")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={load}>{t("work.tryAgain")}</button>
    </div>
  {:else}
    <div class="board" id="board">
      {#each COLS as col (col.id)}
        <section class="col" aria-labelledby="h-{col.id}">
          <div class="col-head">
            <div class="grow">
              <h2 id="h-{col.id}">{t(col.name)} <span class="meta" style="font-weight:600">{byCol[col.id].length}</span></h2>
              <p class="hint-line">{t(col.hint)}</p>
            </div>
          </div>
          <div
            class="col-body"
            class:drop={dropCol === col.id}
            role="list"
            aria-label={t("work.board.colCards", { column: t(col.name) })}
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
              <p class="col-empty">{t(col.empty)}</p>
            {/if}
            {#each byCol[col.id] as card (card.id)}
              {@const s = sessionOf(card)}
              <div
                class="tcard"
                class:needs={s?.status === "waiting"}
                class:dragging={dragId === card.id}
                data-card={card.id}
                role="listitem"
                draggable="true"
                ondragstart={(e) => ondragstart(e, card)}
                ondragend={() => {
                  dragId = null;
                  dropCol = null;
                }}
              >
                <div class="tc-top">
                  {#if card.cli}<CliMark kind={card.cli} small />{/if}
                  <button class="tc-title" type="button" onclick={() => openEdit(card)} title={t("work.board.editCardTitle", { title: card.title })}>{card.title}</button>
                  {#if card.column === "todo"}
                    <button class="tc-act run-act" type="button" aria-label={card.project ? t("work.board.runIn", { folder: folderName(card.project) }) : t("work.board.runInAFolder")} title={t("work.board.run")} onclick={() => openRun(card)}>
                      <Play size={16} aria-hidden="true" />
                    </button>
                  {:else if card.sessionId}
                    <a class="tc-act" href="/session?id={card.sessionId}" aria-label={t("work.board.openSession")} title={t("work.board.openSession")}><ArrowSquareOut size={16} aria-hidden="true" /></a>
                  {/if}
                </div>
                <div class="tc-meta">
                  {#if card.project}
                    <span title={card.project}><FolderSimple size={14} aria-hidden="true" /><span class="mono">{folderName(card.project)}</span></span>
                  {/if}
                  {#if card.column === "progress" && s}
                    {#if s.status === "waiting"}
                      <span class="st-i-wait" title={t("work.board.waiting")}><HandPalm size={14} aria-hidden="true" /><span class="sr-only">{t("work.board.waiting")}</span>{duration(app.now - (s.waiting?.since ?? s.updatedAt))}</span>
                    {:else if s.status === "error"}
                      <span class="st-i-err" title={t("work.board.error")}><WarningCircle size={14} aria-hidden="true" /><span>{t("work.board.error")}</span></span>
                    {:else if s.status === "stopped"}
                      <span title={t("work.board.stopped")}><X size={14} aria-hidden="true" /><span>{t("work.board.stopped")}</span></span>
                    {:else}
                      <span class="st-i-run" title={t("work.board.running")}><SpinnerGap size={14} aria-hidden="true" /><span class="sr-only">{t("work.board.running")}</span>{duration(app.now - s.startedAt)}</span>
                    {/if}
                  {:else if card.column === "done" && s}
                    <span class="st-i-done" title={t("work.board.finished")}><CheckCircle size={14} aria-hidden="true" /><span class="sr-only">{t("work.board.finishedTook")}</span>{duration((s.endedAt ?? s.updatedAt) - s.startedAt)}</span>
                  {:else if card.notes}
                    <span title={t("work.board.hasNotes")}><Note size={14} aria-hidden="true" /><span class="sr-only">{t("work.board.hasNotes")}</span></span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/each}
    </div>
    <p class="meta note" style="margin:0">{t("work.board.dragHint")}</p>
  {/if}
</main>

<Dialog bind:open={cardOpen} labelledby="cd-heading">
  <form class="d-body" novalidate onsubmit={saveCard}>
    <div class="row">
      <h2 id="cd-heading" class="grow">{editing ? t("work.board.editCard") : t("work.board.newCard")}</h2>
      <button class="icon-btn" type="button" aria-label={t("work.close")} onclick={() => (cardOpen = false)}><X size={18} aria-hidden="true" /></button>
    </div>
    <div class="field">
      <label class="label" for="cd-title">{t("work.board.titleLabel")}</label>
      <input class="input" id="cd-title" bind:value={form.title} oninput={() => (titleError = "")} placeholder={t("work.board.titlePlaceholder")} aria-invalid={titleError ? "true" : undefined} aria-describedby={titleError ? "cd-title-error" : undefined} autocomplete="off" />
      {#if titleError}<p class="error" id="cd-title-error">{titleError}</p>{/if}
    </div>
    <div class="field">
      <label class="label" for="cd-notes">{t("work.board.notesLabel")}</label>
      <textarea class="textarea" id="cd-notes" bind:value={form.notes} placeholder={t("work.board.notesPlaceholder")}></textarea>
    </div>
    <FolderField id="cd-project" label={t("work.board.projectLabel")} bind:value={form.project} />
    <div class="opts">
      <div class="field">
        <label class="label" for="cd-cli">{t("work.board.cliLabel")}</label>
        <select class="select" id="cd-cli" bind:value={form.cli}>
          <option value="">{t("work.board.decideLater")}</option>
          {#each app.clis as c (c.kind)}<option value={c.kind} disabled={!c.path}>{c.path ? c.label : t("work.board.cliNotInstalled", { cli: c.label })}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label class="label" for="cd-col">{t("work.board.columnLabel")}</label>
        <select class="select" id="cd-col" bind:value={form.column}>
          {#each COLS as c (c.id)}<option value={c.id}>{t(c.name)}</option>{/each}
        </select>
      </div>
    </div>
    {#if formError}<p class="err-text" role="alert" style="margin:0">{tb(formError)}</p>{/if}
    <div class="d-foot">
      {#if editing}
        <button class="btn ghost danger-text" type="button" onclick={deleteCard}>
          <Trash size={16} aria-hidden="true" /><span>{confirmDelete ? t("work.board.deleteConfirm") : t("work.board.delete")}</span>
        </button>
      {/if}
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (cardOpen = false)}>{t("work.cancel")}</button>
      <button class="btn primary" type="submit" disabled={saving}>{t("work.board.saveCard")}</button>
    </div>
  </form>
</Dialog>

<Dialog bind:open={runOpen} labelledby="rd-title">
  {#if runTask}
    <SessionForm
      idPrefix="rd"
      title={t("work.board.runCard")}
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
    background: var(--solid);
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
