<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
  import { onMount } from "svelte";
  import { api, errorText, type CliKind, type SessionView } from "$lib/api";
  import SessionRow from "$lib/SessionRow.svelte";
  import { CLI_LABEL, folderName, shortPath } from "$lib/format";
  import { app } from "$lib/store.svelte";

  // Every session OpenCompanion ran, searchable (PRD FR-35). The Overview keeps only today's.
  const PAGE = 50;
  const CLIS = Object.entries(CLI_LABEL) as [CliKind, string][];
  const STATUSES = [
    { id: "", label: "Any status" },
    { id: "live", label: "Running" },
    { id: "waiting", label: "Waiting for you" },
    { id: "done", label: "Done" },
    { id: "error", label: "Error" },
    { id: "stopped", label: "Stopped" },
  ];

  let text = $state("");
  let cli = $state("");
  let folder = $state("");
  let status = $state("");
  let rows = $state<SessionView[]>([]);
  let folders = $state<string[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let more = $state(false);
  let asked = 0;

  const filtered = $derived(Boolean(text.trim() || cli || folder || status));
  // Live sessions change while the page is open; the store has their newest state.
  const shown = $derived(rows.map((r) => app.sessions.find((s) => s.id === r.id) ?? r));

  async function search(append = false) {
    // A slower answer to an older search never replaces a newer one.
    const ticket = ++asked;
    try {
      const got = await api.searchSessions({
        text,
        cli: (cli || null) as CliKind | null,
        folder: folder || null,
        status: status || null,
        limit: PAGE,
        offset: append ? rows.length : 0,
      });
      if (ticket !== asked) return;
      rows = append ? [...rows, ...got] : got;
      more = got.length === PAGE;
      loadState = "ready";
    } catch (e) {
      if (ticket !== asked) return;
      loadState = "error";
      loadError = errorText(e);
    }
  }

  // Typing and picking wait a moment, so a word typed quickly is one search, not one per letter.
  $effect(() => {
    void [text, cli, folder, status];
    const t = setTimeout(() => search(), 200);
    return () => clearTimeout(t);
  });

  onMount(() => {
    api
      .sessionFolders()
      .then((f) => (folders = f ?? []))
      .catch(() => undefined);
    const un = listen<string>("session-deleted", (e) => {
      rows = rows.filter((r) => r.id !== e.payload);
    });
    return () => {
      un.then((f) => f());
    };
  });
</script>

<svelte:head><title>All sessions · OpenCompanion</title></svelte:head>

<main class="main" id="history-main">
  <header class="page-head">
    <div class="grow">
      <h1>All sessions</h1>
      <p class="sub">Every session OpenCompanion ran, newest first. Finished ones stay until you delete them or retention in Settings removes them.</p>
    </div>
  </header>

  <div class="toolbar" id="history-toolbar">
    <label class="input-wrap search">
      <MagnifyingGlass size={16} aria-hidden="true" />
      <input type="search" bind:value={text} aria-label="Search sessions" placeholder="Search titles and prompts" spellcheck="false" />
    </label>
    <select class="select" aria-label="CLI" bind:value={cli}>
      <option value="">Every CLI</option>
      {#each CLIS as [kind, label] (kind)}<option value={kind}>{label}</option>{/each}
    </select>
    <select class="select" aria-label="Folder" bind:value={folder}>
      <option value="">Every folder</option>
      {#each folders as f (f)}<option value={f} title={f}>{folderName(f)} · {shortPath(f)}</option>{/each}
    </select>
    <select class="select" aria-label="Status" bind:value={status}>
      {#each STATUSES as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
    </select>
  </div>

  {#if loadState === "loading"}
    <p class="hint" role="status">Looking through your sessions…</p>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>The sessions could not be searched</h2>
      <p>{loadError}</p>
      <button class="btn secondary" type="button" onclick={() => search()}>Try again</button>
    </div>
  {:else if shown.length === 0}
    <p class="hint" role="status">
      {filtered ? "No session matches this search. Clear a filter or try other words." : "Nothing has run in OpenCompanion yet. Sessions you start show up here."}
    </p>
  {:else}
    <section class="section" aria-label="Sessions found">
      <p class="sr-only" role="status">{shown.length === 1 ? "1 session" : `${shown.length}${more ? " or more" : ""} sessions`} found.</p>
      {#each shown as s (s.id)}
        <SessionRow {s} />
      {/each}
    </section>
    {#if more}
      <button class="btn secondary more" type="button" onclick={() => search(true)}>Show {PAGE} more</button>
    {/if}
  {/if}
</main>

<style>
  .toolbar {
    --ink-2: var(--ink);
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .search {
    flex: 1 1 260px;
    min-width: 220px;
  }
  .toolbar .select {
    flex: 0 1 190px;
    width: auto;
    min-width: 150px;
  }
  .more {
    align-self: flex-start;
  }
  @media (max-width: 720px) {
    .toolbar .select {
      flex: 1 1 100%;
    }
  }
</style>
