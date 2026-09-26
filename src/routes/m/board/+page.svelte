<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Check from "phosphor-svelte/lib/Check";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import Play from "phosphor-svelte/lib/Play";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import type { Column, Task } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, folderName, isLive, shortPath, waitingTitle } from "$lib/format";
  import { answer as sendAnswer, call, notify, onMessage, phone } from "$lib/phone.svelte";

  // The desktop Board, one column at a time (PRD FR-77). New cards and edits stay on the desktop.
  const COLS: { id: Column; name: string; empty: string }[] = [
    { id: "pending", name: "Pending", empty: "Nothing pending. New cards made on your computer start here." },
    { id: "todo", name: "Todo", empty: "No ready tasks. A Chat card lands here when you press Add to board." },
    { id: "progress", name: "In progress", empty: "Nothing in progress. Run a Todo card to start a session." },
    { id: "done", name: "Done", empty: "Finished cards land here." },
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
    Object.fromEntries(COLS.map((c) => [c.id, tasks.filter((t) => t.column === c.id).sort((a, b) => a.position - b.position)])) as Record<Column, Task[]>,
  );
  const current = $derived(COLS.find((c) => c.id === col) ?? COLS[1]);
  const opened = $derived(tasks.find((t) => t.id === openId) ?? null);
  const openedSession = $derived(opened ? sessionOf(opened) : undefined);

  // A card deleted on the computer while its sheet is open closes the sheet instead of emptying it.
  $effect(() => {
    if (sheetOpen && openId && !opened) {
      sheetOpen = false;
      notify("That card was deleted on your computer.");
    }
  });

  function sessionOf(t: Task) {
    return t.sessionId ? phone.sessions.find((s) => s.id === t.sessionId) : undefined;
  }

  function pick(c: Column) {
    col = c;
    // Kept in the address, so Back from a session lands on the same column.
    goto(`/m/board?col=${c}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  function openCard(t: Task) {
    openId = t.id;
    failure = "";
    sheetOpen = true;
  }

  async function move(t: Task, to: Column) {
    busy = true;
    failure = "";
    try {
      await call(`/api/tasks/${t.id}/move`, { method: "POST", body: JSON.stringify({ column: to }) });
      await load(true);
      sheetOpen = false;
      notify(`Moved "${t.title}" to ${COLS.find((c) => c.id === to)?.name}.`);
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

<svelte:head><title>Board · OpenCompanion</title></svelte:head>

<header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-board">
  <h1 class="m-h1">Board</h1>

  {#if loadState === "loading"}
    <p class="m-p" role="status">Loading the board…</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">The board could not be loaded: {loadError}</p>
    <button class="btn secondary block" type="button" onclick={() => load()}>Try again</button>
  {:else}
    <div class="m-seg" role="group" aria-label="Board column">
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
      {#each byCol[col] as t (t.id)}
        {@const s = sessionOf(t)}
        <button class="m-card" class:needs={s?.status === "waiting"} type="button" onclick={() => openCard(t)}>
          <span class="row">
            {#if t.cli}<CliMark kind={t.cli} />{/if}
            <span class="task grow">{t.title}</span>
            {#if s}<StatusChip status={s.status} />{/if}
          </span>
          <span class="meta">{t.project ? folderName(t.project) : "No folder yet"}{t.cli ? ` · ${CLI_LABEL[t.cli]}` : ""}{t.notes ? " · has notes" : ""}</span>
        </button>
      {/each}
    </section>
  {/if}
</main>

<Dialog bind:open={sheetOpen} labelledby="card-sheet-title">
  {#if opened}
    {@const s = openedSession}
    <div class="d-body" style="gap:14px">
      <h2 id="card-sheet-title" style="font-size:20px;overflow-wrap:anywhere">{opened.title}</h2>
      <p class="meta" style="margin:0">
        {COLS.find((c) => c.id === opened.column)?.name}{opened.cli ? ` · ${CLI_LABEL[opened.cli]}` : ""}
        {#if opened.project} · <span class="mono" style="overflow-wrap:anywhere">{shortPath(opened.project)}</span>{/if}
      </p>
      {#if opened.notes}<p class="m-notes">{opened.notes}</p>{/if}

      {#if s?.status === "waiting"}
        <section class="m-needs" aria-labelledby="sheet-needs">
          <h3 id="sheet-needs" style="margin:0;font-size:16px;font-weight:800">{waitingTitle(s)}</h3>
          {#if s.waiting?.detail}<div class="cmd"><small>{s.waiting.tool ?? "Request"}</small><code>{s.waiting.detail}</code></div>{/if}
          {#if s.waiting?.canAnswer}
            <div class="pair-btns">
              <button class="btn accent" type="button" disabled={busy} onclick={() => answer(s.id, true)}><Check size={16} aria-hidden="true" />Approve</button>
              <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(s.id, false)}><X size={16} aria-hidden="true" />Deny</button>
            </div>
          {:else}
            <p class="small" style="margin:0">Open the session to answer it.</p>
          {/if}
        </section>
      {/if}

      {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

      <div class="sheet-acts">
        {#if (opened.column === "todo" || opened.column === "pending") && !(s && isLive(s))}
          <a class="btn primary" href="/m/new?task={opened.id}"><Play size={16} aria-hidden="true" />Run {opened.project ? `in ${folderName(opened.project)}` : "this card"}</a>
        {/if}
        {#if opened.sessionId}
          <a class="btn secondary" href="/m/session?id={opened.sessionId}"><TerminalWindow size={16} aria-hidden="true" />Open session</a>
        {/if}
      </div>

      <div class="field">
        <span class="label" id="move-label">Move to</span>
        <div class="move-row" role="group" aria-labelledby="move-label">
          {#each COLS.filter((c) => c.id !== opened.column) as c (c.id)}
            <button class="btn secondary" type="button" disabled={busy} onclick={() => move(opened, c.id)}>{c.name}</button>
          {/each}
        </div>
      </div>

      <button class="btn ghost" type="button" onclick={() => (sheetOpen = false)}>Close</button>
    </div>
  {/if}
</Dialog>
