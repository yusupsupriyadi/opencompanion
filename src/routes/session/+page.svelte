<script lang="ts">
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import Play from "phosphor-svelte/lib/Play";
  import Stop from "phosphor-svelte/lib/Stop";
  import { onMount } from "svelte";
  import { api, errorText, type EventRow, type SessionDetail, type SessionInfo } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import NeedsYou from "$lib/NeedsYou.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import Terminal from "$lib/Terminal.svelte";
  import Timeline from "$lib/Timeline.svelte";
  import { CLI_LABEL, SIGNAL_TEXT, clock, duration, folderName, isLive, modeLabel, shortPath } from "$lib/format";
  import { app, showToast } from "$lib/store.svelte";

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
  let sideOpen = $state(false);

  async function load(target: string) {
    loadState = "loading";
    detail = null;
    events = [];
    if (!target) {
      loadState = "missing";
      return;
    }
    try {
      const d = await api.getSession(target);
      if (target !== id) return;
      detail = d;
      events = d.events;
      loadState = "ready";
    } catch (e) {
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
      if (e.payload.sessionId === id) events = [...events, e.payload];
    });
    return () => {
      un.then((f) => f());
    };
  });

  // The store receives every status change; fall back to the loaded copy.
  const s: SessionInfo | null = $derived(app.sessions.find((x) => x.id === id) ?? detail?.session ?? null);
  const live = $derived(s ? isLive(s) : false);
  const version = $derived(s ? app.clis.find((c) => c.kind === s.cli)?.version : null);
  const marks = $derived(
    events
      .filter((e) => ["tool_call", "file_changed", "permission_request", "permission_denied", "tool_failed", "error"].includes(e.event.kind))
      .map((e) => [e.at, e.event.kind] as [number, string]),
  );
  const files = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of events) if (e.event.kind === "file_changed") counts.set(e.event.path, (counts.get(e.event.path) ?? 0) + 1);
    return [...counts.entries()];
  });

  const canFollowUp = $derived.by(() => {
    if (!s || s.mode !== "headless") return false;
    if (s.status === "waiting") return false;
    if (!live) return Boolean(s.cliSessionId);
    return s.cli === "claude";
  });
  const followUpHint = $derived.by(() => {
    if (!s) return "";
    if (s.status === "waiting") return "Answer the permission request above first";
    if (live && s.cli !== "claude") return `${CLI_LABEL[s.cli]} is working. Send the next message when this turn finishes.`;
    if (!live && !s.cliSessionId) return `${CLI_LABEL[s.cli]} did not report a session id, so this conversation cannot continue`;
    return `Send a follow-up to ${CLI_LABEL[s.cli]}`;
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
      showToast(`Stopping ${CLI_LABEL[s.cli]} in ${folderName(s.cwd)}.`);
    } catch (err) {
      showToast(errorText(err));
    }
  }

  async function resume() {
    if (!s) return;
    resuming = true;
    try {
      await api.resumeSession(s.id, 120, 32);
      termKey += 1;
      detail = await api.getSession(s.id);
      showToast(`Resumed ${CLI_LABEL[s.cli]} in ${folderName(s.cwd)}.`);
    } catch (err) {
      showToast(errorText(err));
    } finally {
      resuming = false;
    }
  }
</script>

<svelte:head><title>{s ? `${s.title} · OpenCompanion` : "Session · OpenCompanion"}</title></svelte:head>

<main class="main detail" id="session-main">
  {#if loadState === "loading"}
    <p class="hint" role="status">Loading the session…</p>
  {:else if loadState === "missing" || !s}
    <div class="not-found">
      <p class="crumb"><a href="/">Overview</a></p>
      <h1>Session not found</h1>
      <p class="sub">It may have been removed with the history in Settings.</p>
      <a class="btn secondary" href="/">Back to Overview</a>
    </div>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>This session could not be loaded</h2>
      <p>{loadError}</p>
      <button class="btn secondary" type="button" onclick={() => load(id)}>Try again</button>
    </div>
  {:else}
    <header class="page-head">
      <div class="grow head-stack">
        <nav class="crumb" aria-label="Breadcrumb"><a href="/">Overview</a> / {folderName(s.cwd)}</nav>
        <div class="row title-row">
          <CliMark kind={s.cli} />
          <h1>{s.title}</h1>
          <StatusChip status={s.status} />
        </div>
        <div class="meta">
          {CLI_LABEL[s.cli]}{version ? ` ${version}` : ""} · {s.mode} · {modeLabel(s.permissionMode)} mode · <span class="mono" title={s.cwd}>{shortPath(s.cwd)}</span> · started {clock(s.startedAt)}
          {#if s.source !== "manual"} · from {s.source === "chat" ? "Chat" : "the Board"}{/if}
        </div>
      </div>
      {#if live}
        <button class="btn danger" type="button" onclick={() => (stopOpen = true)}><Stop size={16} aria-hidden="true" />Stop</button>
      {:else if s.mode === "interactive"}
        <button class="btn primary" type="button" disabled={resuming} onclick={resume}><Play size={16} aria-hidden="true" />{resuming ? "Resuming…" : "Resume"}</button>
      {/if}
    </header>

    {#if s.status === "waiting"}
      <NeedsYou {s} compact showOpen={false} />
    {/if}

    <div class="activity">
      <span class="pixel">Activity</span>
      <Horizon {marks} start={s.startedAt} end={s.endedAt ?? app.now} live={s.status === "running"} />
      <span class="legend">{s.lastEvent ?? "Commands, file edits and approvals show up here"}</span>
      <button class="btn secondary sm side-toggle" type="button" aria-expanded={sideOpen} aria-controls="session-side" onclick={() => (sideOpen = !sideOpen)}>
        {sideOpen ? "Hide details" : "Details"}
      </button>
    </div>

    <div class="detail-body">
      <section class="term" aria-label="Session output">
        {#if s.mode === "interactive"}
          {#key `${id}-${termKey}`}
            <Terminal id={s.id} initial={detail?.output ?? ""} {live} label="Terminal for {CLI_LABEL[s.cli]} in {folderName(s.cwd)}" />
          {/key}
          <div class="term-note">
            {live ? "Type straight into the terminal. Ctrl+C interrupts the CLI." : "This terminal has closed. Resume opens it again with the CLI's own history when it has one."}
          </div>
        {:else}
          <Timeline {events} />
          <form class="term-in" onsubmit={sendFollowUp}>
            <label for="term-input">›</label>
            <input id="term-input" bind:value={followUp} placeholder={followUpHint} disabled={!canFollowUp || sending} autocomplete="off" spellcheck="false" />
            <small>{canFollowUp ? "Enter to send" : ""}</small>
          </form>
          {#if sendError}<div class="term-note" role="alert">{sendError}</div>{/if}
        {/if}
      </section>

      <aside class="detail-side" id="session-side" class:open={sideOpen}>
        <div class="side-group">
          <h3>Files changed</h3>
          {#if files.length === 0}
            <p>{s.mode === "interactive" && s.cli !== "claude" ? "Not reported by this CLI in interactive mode." : "No files changed yet."}</p>
          {:else}
            {#each files as [path, n] (path)}
              <div class="kv"><span class="mono" title={path}>{folderName(path)}</span><b class="mono">{n}×</b></div>
            {/each}
          {/if}
        </div>
        <div class="side-group">
          <h3>Process</h3>
          <div class="kv"><span>Status</span><b>{s.status}</b></div>
          {#if s.pid && live}<div class="kv"><span>PID</span><b class="mono">{s.pid}</b></div>{/if}
          <div class="kv"><span>Running for</span><b>{duration((s.endedAt ?? app.now) - s.startedAt)}</b></div>
          {#if s.exitCode !== null}<div class="kv"><span>Exit code</span><b class="mono">{s.exitCode}</b></div>{/if}
          {#if s.cliSessionId}<div class="kv"><span>CLI session</span><b class="mono" title={s.cliSessionId}>{s.cliSessionId.slice(0, 12)}</b></div>{/if}
        </div>
        <div class="side-group">
          <h3>Needs-you signal</h3>
          <p>
            {#if s.waiting}{SIGNAL_TEXT[s.waiting.method]}
            {:else if s.mode === "headless" && s.cli === "claude"}{SIGNAL_TEXT.stdio}
            {:else if s.mode === "interactive" && s.cli === "claude"}{SIGNAL_TEXT.hook}
            {:else if s.mode === "headless" && s.cli === "opencode"}OpenCode's run mode refuses permission prompts by itself; refusals appear in the output.
            {:else}{SIGNAL_TEXT.screen}{/if}
          </p>
        </div>
        {#if detail?.task}
          <div class="side-group">
            <h3>Board card</h3>
            <p><a class="link" href="/board">{detail.task.title}</a></p>
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</main>

<Dialog bind:open={stopOpen} labelledby="stop-title">
  <div class="d-body">
    <h2 id="stop-title">Stop this session?</h2>
    {#if s}
      <p class="meta" style="margin:0;font-size:14px">
        OpenCompanion sends {CLI_LABEL[s.cli]} in {folderName(s.cwd)} an interrupt first. If it has not exited after 3 seconds, it is stopped by force.
      </p>
    {/if}
    <div class="d-foot">
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (stopOpen = false)}>Keep running</button>
      <button class="btn danger" type="button" onclick={confirmStop}><Stop size={16} aria-hidden="true" />Stop session</button>
    </div>
  </div>
</Dialog>

<style>
  .main.detail {
    gap: 18px;
    padding: 24px 40px 28px;
  }
  .head-stack {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .crumb {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-2);
  }
  .crumb a:hover,
  .link:hover {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .link {
    font-weight: 700;
  }
  .title-row h1 {
    font-size: 24px;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .activity {
    display: flex;
    align-items: center;
    gap: 14px;
    font-size: 12px;
    color: var(--ink-2);
  }
  .activity .legend {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .detail-body {
    flex: 1;
    min-height: 380px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 20px;
  }
  .detail-side {
    display: flex;
    flex-direction: column;
    gap: 22px;
    overflow-y: auto;
  }
  .detail-side p {
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.45;
  }
  .not-found {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
    max-width: 520px;
  }
  .side-toggle {
    display: none;
    margin-left: auto;
  }
  @media (max-width: 1279px) {
    .detail-body {
      grid-template-columns: minmax(0, 1fr);
    }
    .detail-side {
      display: none;
    }
    .detail-side.open {
      display: flex;
    }
    .side-toggle {
      display: inline-flex;
    }
  }
  @media (max-width: 720px) {
    .main.detail {
      padding: 20px 16px;
    }
    .activity .legend {
      display: none;
    }
  }
</style>
