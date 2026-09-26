<script lang="ts">
  import { page } from "$app/state";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import Plus from "phosphor-svelte/lib/Plus";
  import { onMount } from "svelte";
  import CliMark from "$lib/CliMark.svelte";
  import NeedsYou from "$lib/NeedsYou.svelte";
  import SessionRow from "$lib/SessionRow.svelte";
  import { CLI_LABEL, clock, duration, isLive, isToday, memory, shortPath } from "$lib/format";
  import { app, askNewSession, refreshSessions } from "$lib/store.svelte";

  onMount(() => {
    if (page.url.hash === "#new") askNewSession();
  });

  const waiting = $derived(app.sessions.filter((s) => s.status === "waiting"));
  const running = $derived(app.sessions.filter((s) => isLive(s) && s.status !== "waiting"));
  const today = $derived(app.sessions.filter((s) => !isLive(s) && isToday(s.endedAt ?? s.startedAt)));
  const liveCount = $derived(waiting.length + running.length);

  const summary = $derived.by(() => {
    const parts = [`${liveCount} running in OpenCompanion`];
    if (today.length) parts.push(`${today.length} finished today`);
    if (app.outsideState === "ready") parts.push(`${app.outside.length} opened outside`);
    return parts.join(", ");
  });
</script>

<svelte:head><title>Overview · OpenCompanion</title></svelte:head>

<main class="main" id="overview-main">
  <header class="page-head">
    <div class="grow">
      <h1>Overview</h1>
      <p class="sub">{summary}</p>
    </div>
    <button class="btn primary" type="button" id="btn-new-session" onclick={() => askNewSession()}>
      <Plus size={16} aria-hidden="true" />New session
    </button>
  </header>

  {#if app.sessionsState === "loading"}
    <p class="hint" role="status">Loading sessions…</p>
  {:else if app.sessionsState === "error"}
    <div class="state-box" role="alert">
      <h2>Sessions could not be loaded</h2>
      <p>{app.sessionsError}</p>
      <button class="btn secondary" type="button" onclick={refreshSessions}>Try again</button>
    </div>
  {:else}
    {#each waiting as s (s.id)}
      <NeedsYou {s} />
    {/each}

    {#if app.sessions.length === 0}
      <div class="state-box" id="overview-empty">
        <h2>Nothing has run in OpenCompanion yet</h2>
        <p>Start a CLI in one of your project folders, or describe a task in Chat and let the planner suggest where to run it. Sessions you open in other terminals show up below either way.</p>
        <div class="row">
          <button class="btn primary" type="button" onclick={() => askNewSession()}><Plus size={16} aria-hidden="true" />New session</button>
          <a class="btn secondary" href="/chat"><ChatsCircle size={16} aria-hidden="true" />Describe a task in Chat</a>
        </div>
      </div>
    {:else}
      <section class="section" aria-labelledby="running-title">
        <div class="section-head">
          <h2 id="running-title">Running in OpenCompanion</h2>
          <span class="meta">{running.length} running, {today.length} finished today</span>
        </div>
        {#each running as s (s.id)}
          <SessionRow {s} />
        {/each}
        {#each today as s (s.id)}
          <SessionRow {s} />
        {/each}
        {#if running.length === 0 && today.length === 0}
          <p class="hint">Nothing is running right now. Earlier sessions are in the sidebar.</p>
        {/if}
      </section>
    {/if}
  {/if}

  <section class="section" aria-labelledby="outside-title">
    <div class="section-head">
      <h2 id="outside-title">Opened outside OpenCompanion</h2>
      <span class="meta">{app.outsideState === "ready" ? `${app.outside.length} detected` : "Scanning…"}</span>
    </div>
    {#if app.outsideState === "error"}
      <div class="state-box" role="alert">
        <p class="err-text">The process list could not be read: {app.outsideError}</p>
      </div>
    {:else if app.outsideState === "ready" && app.outside.length === 0}
      <p class="hint">No Claude Code, Codex CLI, OpenCode or Gemini CLI is running in another terminal.</p>
    {:else}
      {#each app.outside as x (x.pid)}
        <a class="srow ext" href="/outside?pid={x.pid}">
          <CliMark kind={x.kind} />
          <span style="min-width:0">
            <span class="task">{CLI_LABEL[x.kind]} · {x.mode}</span>
            <span class="where"><span class="mono" title={x.cwd ?? ""}>{x.cwd ? shortPath(x.cwd) : "Folder unknown"}</span></span>
          </span>
          <span class="track">
            <span>Started {clock(x.startedAt * 1000)} · {duration(app.now - x.startedAt * 1000)}</span>
            <span>PID {x.pid} · {memory(x.memoryBytes)}</span>
          </span>
          <span class="status-col"><span class="chip ro">Read-only</span></span>
        </a>
      {/each}
      {#if app.outside.length}
        <p class="hint">Found in the running process list. You can read its transcript here, but only the terminal it was opened in can send it input.</p>
      {/if}
    {/if}
  </section>
</main>
