<script lang="ts">
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import Check from "phosphor-svelte/lib/Check";
  import Play from "phosphor-svelte/lib/Play";
  import Stop from "phosphor-svelte/lib/Stop";
  import { onMount } from "svelte";
  import { api, errorText, type EventRow, type SessionDetail, type SessionInfo, type Usage } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import NeedsYou from "$lib/NeedsYou.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import Terminal from "$lib/Terminal.svelte";
  import Timeline from "$lib/Timeline.svelte";
  import { CLI_LABEL, SIGNAL_TEXT, clock, duration, folderName, isLive, memory, modeLabel, shortPath } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
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
  let markingDone = $state(false);
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
      if (e.payload.sessionId === id && !events.some((x) => x.id === e.payload.id)) events = [...events, e.payload];
    });
    return () => {
      un.then((f) => f());
    };
  });

  // The store receives every status change; fall back to the loaded copy.
  const s: SessionInfo | null = $derived(app.sessions.find((x) => x.id === id) ?? detail?.session ?? null);
  const live = $derived(s ? isLive(s) : false);
  // A terminal that sits idle after its last turn stays open until someone closes it; Done closes
  // it and says the work is finished, where Stop would say it was cut short.
  const canMarkDone = $derived(Boolean(s && s.mode === "interactive" && s.status === "idle"));
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

  async function markDone() {
    if (!s) return;
    markingDone = true;
    try {
      await api.markSessionDone(s.id);
      showToast(t("sessions.detail.markedDone", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) }));
    } catch (err) {
      showToast(tb(errorText(err)));
    } finally {
      markingDone = false;
    }
  }

  async function resume() {
    if (!s) return;
    resuming = true;
    try {
      await api.resumeSession(s.id, 120, 32);
      termKey += 1;
      detail = await api.getSession(s.id);
      showToast(t("sessions.detail.resumed", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) }));
    } catch (err) {
      showToast(tb(errorText(err)));
    } finally {
      resuming = false;
    }
  }
</script>

<svelte:head><title>{s ? s.title : t("sessions.detail.pageTitle")} · OpenCompanion</title></svelte:head>

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
    <header class="page-head">
      <div class="grow head-stack">
        <nav class="crumb" aria-label={t("sessions.breadcrumb")}><a href="/">{t("sessions.overview.title")}</a> / {folderName(s.cwd)}</nav>
        <div class="row title-row">
          <CliMark kind={s.cli} />
          <h1>{s.title}</h1>
          <StatusChip status={s.status} />
        </div>
        <div class="meta">
          {CLI_LABEL[s.cli]}{version ? ` ${version}` : ""} · {t(`sessions.mode.${s.mode}`)} · {t("sessions.detail.permMode", { mode: modeLabel(s.permissionMode) })} · <span class="mono" title={s.cwd}>{shortPath(s.cwd)}</span> · {t("sessions.startedAt", { time: clock(s.startedAt) })}
          {#if s.source !== "manual"} · {s.source === "chat" ? t("sessions.detail.fromChat") : t("sessions.detail.fromBoard")}{/if}
        </div>
      </div>
      {#if live}
        {#if canMarkDone}
          <button class="btn primary" type="button" id="btn-mark-done" disabled={markingDone} title={t("sessions.detail.markDoneHint", { cli: CLI_LABEL[s.cli] })} onclick={markDone}><Check size={16} aria-hidden="true" />{t("sessions.detail.markDone")}</button>
        {/if}
        <button class="btn danger" type="button" onclick={() => (stopOpen = true)}><Stop size={16} aria-hidden="true" />{t("sessions.detail.stop")}</button>
      {:else if s.mode === "interactive"}
        <button class="btn primary" type="button" disabled={resuming} onclick={resume}><Play size={16} aria-hidden="true" />{resuming ? t("sessions.detail.resuming") : t("sessions.detail.resume")}</button>
      {/if}
    </header>

    {#if s.status === "waiting"}
      <NeedsYou {s} compact showOpen={false} />
    {/if}

    <div class="activity">
      <span class="pixel">{t("sessions.detail.activity")}</span>
      <Horizon {marks} start={s.startedAt} end={s.endedAt ?? app.now} live={s.status === "running"} />
      <span class="legend">{s.lastEvent ? tb(s.lastEvent) : t("sessions.detail.activityEmpty")}</span>
      <button class="btn secondary sm side-toggle" type="button" aria-expanded={sideOpen} aria-controls="session-side" onclick={() => (sideOpen = !sideOpen)}>
        {sideOpen ? t("sessions.hideDetails") : t("sessions.details")}
      </button>
    </div>

    <div class="detail-body">
      <section class="term" aria-label={t("sessions.detail.output")}>
        {#if s.mode === "interactive"}
          {#key `${id}-${termKey}`}
            <Terminal id={s.id} {live} label={t("sessions.detail.terminalLabel", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })} />
          {/key}
          <div class="term-note">
            {live ? t("sessions.detail.terminalLive") : t("sessions.detail.terminalClosed")}
          </div>
        {:else}
          <Timeline {events} />
          <form class="term-in" onsubmit={sendFollowUp}>
            <label for="term-input">›</label>
            <input id="term-input" bind:value={followUp} placeholder={followUpHint} disabled={!canFollowUp || sending} autocomplete="off" spellcheck="false" />
            <small>{canFollowUp ? t("sessions.detail.enterToSend") : ""}</small>
          </form>
          {#if sendError}<div class="term-note" role="alert">{tb(sendError)}</div>{/if}
        {/if}
      </section>

      <aside class="detail-side" id="session-side" class:open={sideOpen}>
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
            {:else}{SIGNAL_TEXT.screen}{/if}
          </p>
        </div>
        {#if detail?.task}
          <div class="side-group">
            <h3>{t("sessions.detail.boardCard")}</h3>
            <p><a class="link" href="/board">{detail.task.title}</a></p>
          </div>
        {/if}
      </aside>
    </div>
  {/if}
</main>

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
  @media (max-width: 720px) {
    .activity .legend {
      display: none;
    }
  }
</style>
