<script lang="ts">
  import { page } from "$app/state";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import Plus from "phosphor-svelte/lib/Plus";
  import { onMount } from "svelte";
  import CliMark from "$lib/CliMark.svelte";
  import NeedsYou from "$lib/NeedsYou.svelte";
  import SessionRow from "$lib/SessionRow.svelte";
  import { CLI_LABEL, clock, duration, isLive, isToday, memory, runsCli, shortPath } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { app, askNewSession, refreshSessions } from "$lib/store.svelte";

  onMount(() => {
    if (page.url.hash === "#new") askNewSession();
  });

  const waiting = $derived(app.sessions.filter((s) => s.status === "waiting"));
  const running = $derived(app.sessions.filter((s) => runsCli(s) && s.status !== "waiting"));
  // Terminals left at their shell prompt: open, but nothing runs in them.
  const atPrompt = $derived(app.sessions.filter((s) => s.status === "shell"));
  const today = $derived(app.sessions.filter((s) => !isLive(s) && isToday(s.endedAt ?? s.startedAt)));
  const liveCount = $derived(waiting.length + running.length);

  const summary = $derived.by(() => {
    const parts = [t("sessions.overview.summaryRunning", { n: liveCount })];
    if (today.length) parts.push(t("sessions.overview.summaryFinished", { n: today.length }));
    if (app.outsideState === "ready") parts.push(t("sessions.overview.summaryOutside", { n: app.outside.length }));
    return parts.join(", ");
  });
</script>

<svelte:head><title>{t("sessions.overview.title")} · OpenCompanion</title></svelte:head>

<main class="main" id="overview-main">
  <header class="page-head">
    <div class="grow">
      <h1>{t("sessions.overview.title")}</h1>
      <p class="sub">{summary}</p>
    </div>
    <button class="btn primary" type="button" id="btn-new-session" onclick={() => askNewSession()}>
      <Plus size={16} aria-hidden="true" />{t("sessions.overview.newSession")}
    </button>
  </header>

  {#if app.sessionsState === "loading"}
    <p class="hint" role="status">{t("sessions.overview.loading")}</p>
  {:else if app.sessionsState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("sessions.overview.loadFailed")}</h2>
      <p>{tb(app.sessionsError)}</p>
      <button class="btn secondary" type="button" onclick={refreshSessions}>{t("sessions.tryAgain")}</button>
    </div>
  {:else}
    {#each waiting as s (s.id)}
      <NeedsYou {s} />
    {/each}

    {#if app.sessions.length === 0}
      <div class="state-box" id="overview-empty">
        <h2>{t("sessions.overview.emptyTitle")}</h2>
        <p>{t("sessions.overview.emptyBody")}</p>
        <div class="row">
          <button class="btn primary" type="button" onclick={() => askNewSession()}><Plus size={16} aria-hidden="true" />{t("sessions.overview.newSession")}</button>
          <a class="btn secondary" href="/chat"><ChatsCircle size={16} aria-hidden="true" />{t("sessions.overview.describeInChat")}</a>
        </div>
      </div>
    {:else}
      <section class="section" aria-labelledby="running-title">
        <div class="section-head">
          <h2 id="running-title">{t("sessions.overview.runningTitle")}</h2>
          <span class="meta">{t("sessions.overview.runningMeta", { running: running.length, today: today.length })}</span>
          <a class="link" href="/history" id="link-all-sessions">{t("sessions.overview.allSessions")}</a>
        </div>
        {#each running as s (s.id)}
          <SessionRow {s} />
        {/each}
        {#each atPrompt as s (s.id)}
          <SessionRow {s} />
        {/each}
        {#each today as s (s.id)}
          <SessionRow {s} />
        {/each}
        {#if running.length === 0 && atPrompt.length === 0 && today.length === 0}
          {@const [before, after] = t("sessions.overview.nothingRunning").split("{link}")}
          <p class="hint">{before}<a class="link" href="/history">{t("sessions.overview.allSessions")}</a>{after}</p>
        {/if}
      </section>
    {/if}
  {/if}

  <section class="section" aria-labelledby="outside-title">
    <div class="section-head">
      <h2 id="outside-title">{t("sessions.openedOutside")}</h2>
      <span class="meta">{app.outsideState === "ready" ? t("sessions.overview.detected", { n: app.outside.length }) : t("sessions.overview.scanning")}</span>
    </div>
    {#if app.outsideState === "error"}
      <div class="state-box" role="alert">
        <p class="err-text">{t("sessions.overview.scanFailed", { error: app.outsideError })}</p>
      </div>
    {:else if app.outsideState === "ready" && app.outside.length === 0}
      <p class="hint">{t("sessions.overview.noneOutside")}</p>
    {:else}
      {#each app.outside as x (x.pid)}
        <a class="srow ext" href="/outside?pid={x.pid}">
          <CliMark kind={x.kind} />
          <span style="min-width:0">
            <span class="task">{CLI_LABEL[x.kind]} · {t(`sessions.mode.${x.mode}`)}</span>
            <span class="where"><span class="mono" title={x.cwd ?? ""}>{x.cwd ? shortPath(x.cwd) : t("sessions.folderUnknown")}</span></span>
          </span>
          <span class="track">
            <span>{t("sessions.overview.outsideStarted", { time: clock(x.startedAt * 1000), duration: duration(app.now - x.startedAt * 1000) })}</span>
            <span>{t("sessions.overview.outsidePid", { pid: x.pid, memory: memory(x.memoryBytes) })}</span>
          </span>
          <span class="status-col"><span class="chip ro">{t("sessions.readOnly")}</span></span>
        </a>
      {/each}
      {#if app.outside.length}
        <p class="hint">{t("sessions.overview.outsideHint")}</p>
      {/if}
    {/if}
  </section>
</main>
