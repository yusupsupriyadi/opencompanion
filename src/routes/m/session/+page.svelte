<script lang="ts">
  import { page } from "$app/state";
  import ArrowDown from "phosphor-svelte/lib/ArrowDown";
  import ArrowElbowDownLeft from "phosphor-svelte/lib/ArrowElbowDownLeft";
  import ArrowUp from "phosphor-svelte/lib/ArrowUp";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import Check from "phosphor-svelte/lib/Check";
  import Info from "phosphor-svelte/lib/Info";
  import ListBullets from "phosphor-svelte/lib/ListBullets";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Play from "phosphor-svelte/lib/Play";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Stop from "phosphor-svelte/lib/Stop";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import type { EventRow, SessionInfo, Task, Usage } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import PhoneScroll from "$lib/PhoneScroll.svelte";
  import PhoneStatus from "$lib/PhoneStatus.svelte";
  import { CLI_LABEL, SIGNAL_TEXT, clock, duration, folderName, isLive, memory, modeLabel, shortPath, waitingTitle } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { answer as sendAnswer, call, notify, onMessage, phone } from "$lib/phone.svelte";
  import { parseTail } from "$lib/tail";
  import { timelineLines } from "$lib/timeline";

  type Detail = {
    session: SessionInfo;
    events: EventRow[];
    tail: string;
    /** `[path, edits]`, in the order first changed. */
    files?: [string, number][];
    task?: Task | null;
    usage?: Usage | null;
  };
  type View = "terminal" | "activity" | "details";

  const id = $derived(page.url.searchParams.get("id") ?? "");

  // The keys a terminal UI asks for: cancel, move focus, move through a list, confirm, interrupt.
  // An arrow or the return mark says it alone; the rest keep their key cap. Names are getters, so
  // they follow the UI language.
  const KEYS = [
    { key: "esc", cap: "Esc", get name() { return t("phone.session.keyEsc"); } },
    { key: "tab", cap: "Tab", get name() { return t("phone.session.keyTab"); } },
    { key: "up", icon: ArrowUp, get name() { return t("phone.session.keyUp"); } },
    { key: "down", icon: ArrowDown, get name() { return t("phone.session.keyDown"); } },
    { key: "enter", icon: ArrowElbowDownLeft, get name() { return t("phone.session.keyEnter"); } },
    { key: "ctrl_c", cap: "Ctrl+C", get name() { return t("phone.session.keyCtrlC"); } },
  ];

  let loaded = $state<Detail | null>(null);
  let loadState = $state<"loading" | "ready" | "missing" | "error">("loading");
  let loadError = $state("");
  let failure = $state("");
  let busy = $state(false);
  let stopOpen = $state(false);
  let text = $state("");
  let sending = $state(false);
  let sendError = $state("");
  let view = $state<View | null>(null);
  let now = $state(Date.now());

  async function load() {
    try {
      loaded = await call<Detail>(`/api/sessions/${id}`);
      loadState = "ready";
    } catch (e) {
      if (e instanceof Error && "status" in e && (e as { status: number }).status === 404) loadState = "missing";
      // A failed refresh keeps the screen as it was; the offline screen covers a desktop that went away.
      else if (!loaded) {
        loadState = "error";
        loadError = e instanceof Error ? e.message : String(e);
      }
    }
  }

  function retry() {
    loadState = "loading";
    load();
  }

  let pending: ReturnType<typeof setTimeout> | undefined;
  function reloadSoon(ms = 400) {
    clearTimeout(pending);
    pending = setTimeout(load, ms);
  }

  onMount(() => {
    load();
    const off = onMessage((m) => {
      // Refresh the screen and events shortly after this session changes.
      const sid = m.session?.id ?? m.event?.sessionId;
      if (sid === id) reloadSoon();
    });
    // A terminal's screen, CPU and memory are not pushed to the phone, so a live session is read more often.
    const timer = setInterval(() => {
      if (s && isLive(s)) load();
    }, 2500);
    const clockTimer = setInterval(() => (now = Date.now()), 15_000);
    return () => {
      off();
      clearInterval(timer);
      clearInterval(clockTimer);
      clearTimeout(pending);
    };
  });

  const s = $derived(phone.sessions.find((x) => x.id === id) ?? loaded?.session ?? null);
  const live = $derived(s ? isLive(s) : false);
  const terminal = $derived(s?.mode === "interactive");
  const canMarkDone = $derived(Boolean(s && s.mode === "interactive" && s.status === "idle"));
  const answerable = $derived(Boolean(s && s.status === "waiting" && s.waiting?.canAnswer));
  const events = $derived(loaded?.events ?? []);
  const files = $derived(loaded?.files ?? []);
  const usage = $derived(live ? (loaded?.usage ?? null) : null);
  const blocks = $derived(parseTail(loaded?.tail ?? ""));
  const marks = $derived(
    events
      .filter((e) => ["tool_call", "file_changed", "permission_request", "permission_denied", "tool_failed", "error"].includes(e.event.kind))
      .map((e) => [e.at, e.event.kind] as [number, string]),
  );

  // A terminal opens on its screen; a headless session, whose steps are its output, on Activity.
  const views = $derived.by(() => {
    const all = [
      { id: "terminal" as View, icon: TerminalWindow, label: t("phone.session.terminal") },
      { id: "activity" as View, icon: ListBullets, label: t("sessions.detail.activity") },
      { id: "details" as View, icon: Info, label: t("phone.session.tabDetails") },
    ];
    return terminal ? all : all.slice(1);
  });
  const shown = $derived(view && views.some((v) => v.id === view) ? view : views[0].id);

  // The same rule as the desktop Session screen.
  const canFollowUp = $derived.by(() => {
    if (!s || s.mode !== "headless" || s.status === "waiting") return false;
    if (!live) return Boolean(s.cliSessionId);
    return s.cli === "claude";
  });
  const followUpHint = $derived.by(() => {
    if (!s) return "";
    if (s.status === "waiting") return t("phone.session.hintWaiting");
    if (live && s.cli !== "claude") return t("phone.session.hintWorking", { cli: CLI_LABEL[s.cli] });
    if (!live && !s.cliSessionId) return t("phone.session.hintNoId", { cli: CLI_LABEL[s.cli] });
    return t("phone.session.hintNext", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) });
  });

  const signal = $derived.by(() => {
    if (!s) return "";
    if (s.waiting) return SIGNAL_TEXT[s.waiting.method];
    if (s.mode === "headless" && s.cli === "claude") return SIGNAL_TEXT.stdio;
    if (s.mode === "interactive" && s.cli === "claude") return SIGNAL_TEXT.hook;
    if (s.mode === "headless" && s.cli === "opencode") return t("sessions.detail.signalOpenCode");
    if (s.cli === "pi") return t("sessions.detail.signalPi");
    if (s.cli === "omp") return t("sessions.detail.signalOmp");
    return SIGNAL_TEXT.screen;
  });

  async function answer(allow: boolean) {
    busy = true;
    failure = "";
    try {
      await sendAnswer(id, allow);
      await load();
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const message = text.trim();
    if (!message || sending) return;
    sending = true;
    sendError = "";
    try {
      await call(`/api/sessions/${id}/input`, { method: "POST", body: JSON.stringify({ text: message }) });
      text = "";
      reloadSoon(terminal ? 600 : 400);
    } catch (err) {
      sendError = err instanceof Error ? err.message : String(err);
    } finally {
      sending = false;
    }
  }

  async function press(key: string) {
    sendError = "";
    try {
      await call(`/api/sessions/${id}/input`, { method: "POST", body: JSON.stringify({ key }) });
      reloadSoon(300);
    } catch (err) {
      sendError = err instanceof Error ? err.message : String(err);
    }
  }

  async function resume() {
    if (!s) return;
    busy = true;
    sendError = "";
    try {
      await call(`/api/sessions/${id}/resume`, { method: "POST" });
      notify(t("phone.session.resumed", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) }));
      reloadSoon(800);
    } catch (err) {
      sendError = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  // Resume undoes it, so unlike Stop it goes without a confirmation sheet.
  async function markDone() {
    if (!s) return;
    busy = true;
    failure = "";
    try {
      await call(`/api/sessions/${id}/done`, { method: "POST" });
      notify(t("phone.session.markedDone", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) }));
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function stop() {
    stopOpen = false;
    busy = true;
    try {
      await call(`/api/sessions/${id}/stop`, { method: "POST" });
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>{s ? s.title : t("phone.session.title")} · OpenCompanion</title></svelte:head>

<!-- The screen is exactly one phone tall: the header and the dock stay, the output between them scrolls. -->
<div class="m-session" id="phone-session-screen">
  <header class="m-shead" id="phone-session-head">
    <div class="m-shead-row">
      <a class="m-icon-btn" href="/m" aria-label={t("phone.session.back")} title={t("phone.session.back")}><CaretLeft size={20} aria-hidden="true" /></a>
      {#if s}
        <CliMark kind={s.cli} small />
        <h1 class="m-stitle">{s.title}</h1>
        <span role="status"><PhoneStatus status={s.status} /></span>
        {#if canMarkDone}
          <button class="m-icon-btn secondary m-done" type="button" id="btn-mark-done" aria-label={t("phone.session.markDone")} title={t("phone.session.markDone")} disabled={busy} onclick={markDone}>
            <Check size={18} weight="bold" aria-hidden="true" />
          </button>
        {/if}
        {#if live}
          <button class="m-icon-btn danger" type="button" id="btn-stop-session" aria-label={t("phone.session.stop")} title={t("phone.session.stop")} disabled={busy} onclick={() => (stopOpen = true)}>
            <Stop size={18} weight="fill" aria-hidden="true" />
          </button>
        {/if}
      {/if}
    </div>
    {#if s}
      <p class="m-smeta">{CLI_LABEL[s.cli]} · {t(`phone.mode.${s.mode}`)} · {duration((s.endedAt ?? now) - s.startedAt)} · <span class="mono">{folderName(s.cwd)}</span></p>
      <Horizon {marks} start={s.startedAt} end={s.endedAt ?? now} live={s.status === "running"} full />
    {/if}
  </header>

  <main class="m-sbody" id="phone-session">
    {#if loadState === "loading" && !s}
      <p class="m-p m-loading" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("phone.session.loading")}</p>
    {:else if loadState === "error" && !s}
      <p class="err-text" role="alert" style="margin:0">{t("phone.session.loadFailed", { error: tb(loadError) })}</p>
      <button class="btn secondary block" type="button" onclick={retry}>{t("phone.tryAgain")}</button>
    {:else if !s}
      <h1 class="m-h1">{t("phone.session.missing")}</h1>
      <p class="m-p">{t("phone.session.missingBody")}</p>
      <a class="btn secondary block" href="/m">{t("phone.session.back")}</a>
    {:else}
      {#if s.status === "waiting"}
        <section class="m-needs m-appear" aria-labelledby="ms-needs">
          <h2 id="ms-needs">{waitingTitle(s)}</h2>
          {#if s.waiting?.detail}<div class="cmd m-cmd"><small>{s.waiting.tool ?? t("phone.request")}</small><code>{s.waiting.detail}</code></div>{/if}
          {#if !s.waiting?.canAnswer}
            <p class="small" style="margin:0">{terminal ? t("phone.session.answerWithKeys") : t("phone.answerOnComputer")}</p>
          {/if}
        </section>
      {/if}

      {#if failure}<p class="err-text m-appear" role="alert" style="margin:0">{tb(failure)}</p>{/if}

      <div class="m-seg m-views" role="group" aria-label={t("phone.session.views")}>
        {#each views as v (v.id)}
          <button type="button" aria-pressed={shown === v.id} onclick={() => (view = v.id)}><v.icon size={16} aria-hidden="true" />{v.label}</button>
        {/each}
      </div>

      {#if shown === "terminal"}
        <PhoneScroll label={t("phone.session.screen")} version={loaded?.tail}>
          <pre class="m-screen">{#each blocks as b, i (i)}{#if b.kind === "text"}<span class="m-tx">{b.text}</span>{:else if b.kind === "rule"}<span class="m-rule" aria-hidden="true"></span>{:else}<span class="m-box">{#if b.title}<span class="m-box-title">{b.title}</span>{/if}{#each b.lines as l, j (j)}{#if l === null}<span class="m-rule" aria-hidden="true"></span>{:else}<span class="m-tx">{l}</span>{/if}{/each}</span>{/if}{:else}<span class="t-d">{t("phone.session.noOutput")}</span>{/each}</pre>
        </PhoneScroll>
      {:else if shown === "activity"}
        <PhoneScroll label={t("sessions.detail.activity")} version={events.at(-1)?.id}>
          <pre class="m-screen" aria-live="polite">{#each events as row (row.id)}{#each timelineLines(row) as l, j (j)}<span class="m-tx {l.cls}">{l.text}</span>{/each}{:else}<span class="t-d">{live ? t("sessions.timeline.waiting") : t("phone.session.noOutput")}</span>{/each}</pre>
        </PhoneScroll>
      {:else}
        <!-- It scrolls on its own, so a keyboard has to be able to reach it. -->
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="m-details" role="region" aria-label={t("phone.session.tabDetails")} tabindex="0">
          <section class="side-group" aria-labelledby="md-session">
            <h2 id="md-session">{t("phone.session.title")}</h2>
            <div class="kv"><span>{t("phone.session.folder")}</span><b class="mono">{shortPath(s.cwd)}</b></div>
            <div class="kv"><span>CLI</span><b>{CLI_LABEL[s.cli]} · {t(`phone.mode.${s.mode}`)} · {t("sessions.detail.permMode", { mode: modeLabel(s.permissionMode) })}</b></div>
            <div class="kv">
              <span>{t("phone.session.startedAt")}</span>
              <b>{clock(s.startedAt)}{#if s.source !== "manual"} · {s.source === "chat" ? t("sessions.detail.fromChat") : t("sessions.detail.fromBoard")}{/if}</b>
            </div>
            <div class="kv"><span>{t("sessions.runningFor")}</span><b>{duration((s.endedAt ?? now) - s.startedAt)}</b></div>
          </section>

          <section class="side-group" aria-labelledby="md-process">
            <h2 id="md-process">{t("sessions.process")}</h2>
            <div class="kv"><span>{t("sessions.detail.status")}</span><b>{t(`sessions.status.${s.status}`)}</b></div>
            {#if s.pid && live}<div class="kv"><span>{t("sessions.pid")}</span><b class="mono">{s.pid}</b></div>{/if}
            {#if usage}
              <div class="kv"><span>{t("sessions.cpu")}</span><b class="mono">{usage.cpuPercent}%</b></div>
              <div class="kv"><span>{t("sessions.memory")}</span><b class="mono">{memory(usage.memoryBytes)}</b></div>
              <div class="kv"><span>{t("sessions.detail.childProcesses")}</span><b class="mono">{usage.children}</b></div>
            {:else if live && s.pid}
              <p class="small" style="margin:0">{t("sessions.detail.measuring")}</p>
            {/if}
            {#if s.exitCode !== null}<div class="kv"><span>{t("sessions.detail.exitCode")}</span><b class="mono">{s.exitCode}</b></div>{/if}
            {#if s.cliSessionId}<div class="kv"><span>{t("sessions.detail.cliSession")}</span><b class="mono">{s.cliSessionId.slice(0, 12)}</b></div>{/if}
          </section>

          <section class="side-group" aria-labelledby="md-files">
            <h2 id="md-files">{t("sessions.detail.filesChanged")}{files.length ? ` · ${files.length}` : ""}</h2>
            {#each files as [path, n] (path)}
              <div class="kv"><span class="mono">{folderName(path)}</span><b class="mono">{n}×</b></div>
            {:else}
              <p class="small" style="margin:0">{terminal && s.cli !== "claude" ? t("sessions.detail.filesNotReported") : t("sessions.detail.noFiles")}</p>
            {/each}
          </section>

          <section class="side-group" aria-labelledby="md-signal">
            <h2 id="md-signal">{t("sessions.detail.signal")}</h2>
            <p class="small" style="margin:0">{signal}</p>
          </section>

          {#if loaded?.task}
            <section class="side-group" aria-labelledby="md-card">
              <h2 id="md-card">{t("sessions.detail.boardCard")}</h2>
              <a class="link" href="/m/board?col={loaded.task.column}">{loaded.task.title}</a>
            </section>
          {/if}
        </div>
      {/if}
    {/if}
  </main>

  {#if s}
    <div class="m-sdock" id="phone-session-dock">
      {#if answerable}
        <div class="pair-btns">
          <button class="btn accent" type="button" disabled={busy} onclick={() => answer(true)}><Check size={16} aria-hidden="true" />{t("phone.approve")}</button>
          <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(false)}><X size={16} aria-hidden="true" />{t("phone.deny")}</button>
        </div>
      {:else if terminal && live}
        <form class="m-dock-stack" onsubmit={send}>
          <div class="m-keys" role="group" aria-label={t("phone.session.keys")}>
            {#each KEYS as k (k.key)}
              <button class="m-key" type="button" aria-label={k.name} title={k.name} onclick={() => press(k.key)}>
                {#if k.icon}<k.icon size={16} weight="bold" aria-hidden="true" />{:else}{k.cap}{/if}
              </button>
            {/each}
          </div>
          <div class="send-row">
            <label class="sr-only" for="ms-input">{t("phone.session.typeLabel")}</label>
            <input class="input" id="ms-input" bind:value={text} enterkeyhint="send" autocomplete="off" autocapitalize="off" spellcheck="false" placeholder={t("phone.session.typePlaceholder")} />
            <button class="m-icon-btn primary lg" type="submit" aria-label={t("phone.send")} title={t("phone.send")} disabled={sending || !text.trim()}>
              {#if sending}<SpinnerGap size={18} class="spin" aria-hidden="true" />{:else}<PaperPlaneTilt size={18} aria-hidden="true" />{/if}
            </button>
          </div>
          {#if sendError}<p class="err-text small m-appear" role="alert">{tb(sendError)}</p>{/if}
        </form>
      {:else if terminal}
        <div class="m-dock-stack">
          <button class="btn primary" type="button" id="btn-resume-session" disabled={busy} onclick={resume}>
            {#if busy}<SpinnerGap size={16} class="spin" aria-hidden="true" />{:else}<Play size={16} aria-hidden="true" />{/if}{busy ? t("phone.session.resuming") : t("phone.session.resume")}
          </button>
          {#if sendError}<p class="err-text small m-appear" role="alert">{tb(sendError)}</p>{:else}<p class="small">{t("phone.session.resumeHelp")}</p>{/if}
        </div>
      {:else}
        <form class="m-dock-stack" onsubmit={send}>
          <div class="send-row">
            <label class="sr-only" for="ms-input">{t("phone.session.messageFor", { cli: CLI_LABEL[s.cli] })}</label>
            <textarea class="textarea" id="ms-input" rows="1" bind:value={text} disabled={!canFollowUp} aria-describedby="ms-hint" placeholder={canFollowUp ? t("phone.session.followUpFor", { cli: CLI_LABEL[s.cli] }) : t("phone.session.unavailable")}></textarea>
            <button class="m-icon-btn primary lg" type="submit" aria-label={t("phone.send")} title={t("phone.send")} disabled={!canFollowUp || sending || !text.trim()}>
              {#if sending}<SpinnerGap size={18} class="spin" aria-hidden="true" />{:else}<PaperPlaneTilt size={18} aria-hidden="true" />{/if}
            </button>
          </div>
          {#if sendError}<p class="err-text small m-appear" role="alert">{tb(sendError)}</p>{:else}<p class="small" id="ms-hint" class:sr-only={canFollowUp}>{followUpHint}</p>{/if}
        </form>
      {/if}
    </div>
  {/if}
</div>

<Dialog bind:open={stopOpen} labelledby="stop-sheet-title" sheet>
  {#if s}
    <div class="d-body" style="gap:14px">
      <h2 id="stop-sheet-title" style="font-size:20px">{t("phone.session.stopTitle")}</h2>
      <p class="m-p">{t("phone.session.stopBody", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })}</p>
      <div class="d-foot">
        <button class="btn danger" type="button" onclick={stop}><Stop size={16} aria-hidden="true" />{t("phone.session.stopConfirm")}</button>
        <button class="btn secondary" type="button" onclick={() => (stopOpen = false)}>{t("phone.session.keepRunning")}</button>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .m-session {
    display: flex;
    flex-direction: column;
    height: 100vh;
    height: 100dvh;
  }
  .m-shead {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: calc(4px + env(safe-area-inset-top)) 12px 0;
    background: var(--bg);
  }
  .m-shead-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
  }
  .m-shead-row > a {
    margin-left: -6px;
  }
  .m-done:not([disabled]) {
    color: var(--forest);
  }
  .m-stitle {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 15px;
    font-weight: 800;
    line-height: 1.25;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  /* Starts under the CLI mark, so the title, the mark and this line read as one block. */
  .m-smeta {
    margin: 0 0 0 42px;
    font-size: 12px;
    color: var(--ink-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .m-smeta .mono {
    font-size: 12px;
  }
  /* Scrolls as a whole only when a phone held sideways leaves the pane less than its minimum. */
  .m-sbody {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 12px 8px;
  }
  .m-loading {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-cmd {
    padding: 8px 10px;
    max-height: 7.5em;
    overflow-y: auto;
  }
  .m-cmd code {
    font-size: 13px;
  }
  .m-details {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 6px 2px 8px;
  }
  .m-details h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 800;
  }
  .m-details .kv b {
    min-width: 0;
    text-align: right;
    overflow-wrap: anywhere;
  }
  .m-sdock {
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 8px 12px calc(8px + env(safe-area-inset-bottom));
    border-top: 1px solid var(--line);
    background: var(--bg);
  }
  .m-sdock .pair-btns .btn,
  .m-sdock .m-dock-stack > .btn {
    min-height: 44px;
  }
  .m-sdock .send-row {
    align-items: flex-end;
  }
  .m-sdock .send-row .input,
  .m-sdock .send-row .textarea {
    min-height: 44px;
  }
  .m-sdock .send-row .textarea {
    max-height: 132px;
    padding-top: 11px;
    padding-bottom: 11px;
    resize: none;
    field-sizing: content;
  }
</style>
