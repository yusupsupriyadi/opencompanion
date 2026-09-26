<script lang="ts">
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import Check from "phosphor-svelte/lib/Check";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Play from "phosphor-svelte/lib/Play";
  import Stop from "phosphor-svelte/lib/Stop";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import type { EventRow, SessionInfo } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, clock, folderName, isLive, shortPath, waitingTitle } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { answer as sendAnswer, call, notify, onMessage, phone } from "$lib/phone.svelte";

  const id = $derived(page.url.searchParams.get("id") ?? "");

  // The keys a terminal UI asks for: confirm, cancel, move through a list, interrupt.
  // The spoken names are getters, so they follow the UI language; the key caps stay as printed.
  const KEYS = [
    { key: "enter", label: "Enter", get name() { return t("phone.session.keyEnter"); } },
    { key: "esc", label: "Esc", get name() { return t("phone.session.keyEsc"); } },
    { key: "up", label: "↑", get name() { return t("phone.session.keyUp"); } },
    { key: "down", label: "↓", get name() { return t("phone.session.keyDown"); } },
    { key: "ctrl_c", label: "Ctrl+C", get name() { return t("phone.session.keyCtrlC"); } },
  ];

  let loaded = $state<{ session: SessionInfo; events: EventRow[]; tail: string } | null>(null);
  let loadState = $state<"loading" | "ready" | "missing">("loading");
  let failure = $state("");
  let busy = $state(false);
  let stopOpen = $state(false);
  let text = $state("");
  let sending = $state(false);
  let sendError = $state("");
  let dockHeight = $state(0);

  async function load() {
    try {
      loaded = await call(`/api/sessions/${id}`);
      loadState = "ready";
    } catch (e) {
      if (e instanceof Error && "status" in e && (e as { status: number }).status === 404) loadState = "missing";
      else failure = e instanceof Error ? e.message : String(e);
    }
  }

  let pending: ReturnType<typeof setTimeout> | undefined;
  function reloadSoon(ms = 400) {
    clearTimeout(pending);
    pending = setTimeout(load, ms);
  }

  onMount(() => {
    load();
    const off = onMessage((m) => {
      // Refresh the tail and events shortly after this session changes.
      const sid = m.session?.id ?? m.event?.sessionId;
      if (sid === id) reloadSoon();
    });
    // A terminal's screen is not pushed to the phone, so a live one is read more often.
    const timer = setInterval(() => {
      if (s && isLive(s)) load();
    }, 2500);
    return () => {
      off();
      clearInterval(timer);
      clearTimeout(pending);
    };
  });

  const s = $derived(phone.sessions.find((x) => x.id === id) ?? loaded?.session ?? null);
  const live = $derived(s ? isLive(s) : false);
  const terminal = $derived(s?.mode === "interactive");
  const answerable = $derived(Boolean(s && s.status === "waiting" && s.waiting?.canAnswer));

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

  const lines = $derived.by(() => {
    if (!loaded) return "";
    if (loaded.tail.trim()) return loaded.tail;
    return loaded.events
      .slice(-30)
      .map((e) => {
        const ev = e.event;
        switch (ev.kind) {
          case "message":
            return ev.text;
          case "tool_call":
            return `• ${ev.tool} ${ev.summary}`;
          case "file_changed":
            return `  ${t("phone.session.logEdited", { path: ev.path })}`;
          case "permission_request":
            return `? ${ev.tool}: ${ev.summary}`;
          case "permission_denied":
            return `✗ ${t("phone.session.logRefused", { tool: ev.tool })}`;
          case "tool_failed":
            return `✗ ${t("phone.session.logFailed", { tool: ev.tool, message: ev.message })}`;
          case "error":
            return ev.message;
          case "done":
            return ev.ok ? `✓ ${t("phone.session.logFinished", { time: clock(e.at) })}` : `✗ ${t("phone.session.logDoneFailed", { summary: ev.summary })}`;
          default:
            return "";
        }
      })
      .filter(Boolean)
      .join("\n");
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

<header class="bar">
  <a class="back grow" href="/m"><CaretLeft size={20} aria-hidden="true" />{t("phone.nav.sessions")}</a>
  {#if s && live}
    <button class="btn danger" type="button" id="btn-stop-session" disabled={busy} onclick={() => (stopOpen = true)}><Stop size={16} aria-hidden="true" />{t("phone.session.stop")}</button>
  {/if}
</header>
<main class="content" id="phone-session" style:padding-bottom={s ? `${dockHeight + 24}px` : undefined}>
  {#if loadState === "loading" && !s}
    <p class="m-p" role="status">{t("phone.session.loading")}</p>
  {:else if loadState === "missing" || !s}
    <h1 class="m-h1">{t("phone.session.missing")}</h1>
    <p class="m-p">{t("phone.session.missingBody")}</p>
    <a class="btn secondary block" href="/m">{t("phone.session.back")}</a>
  {:else}
    <div class="row" style="align-items:flex-start">
      <CliMark kind={s.cli} />
      <h1 class="m-title grow">{s.title}</h1>
    </div>
    <div class="row" style="gap:8px;flex-wrap:wrap">
      <StatusChip status={s.status} />
      <span class="meta">{CLI_LABEL[s.cli]} · {t(`phone.mode.${s.mode}`)} · {t("phone.session.started", { ago: ago(s.startedAt) })}</span>
    </div>
    <p class="meta mono" style="margin:0;overflow-wrap:anywhere">{shortPath(s.cwd)}</p>

    {#if s.status === "waiting"}
      <section class="m-needs" aria-labelledby="ms-needs">
        <h2 id="ms-needs">{waitingTitle(s)}</h2>
        {#if s.waiting?.detail}<div class="cmd"><small>{s.waiting.tool ?? t("phone.request")}</small><code>{s.waiting.detail}</code></div>{/if}
        {#if !s.waiting?.canAnswer}
          <p class="small" style="margin:0">
            {terminal ? t("phone.session.answerWithKeys") : t("phone.answerOnComputer")}
          </p>
        {/if}
      </section>
    {/if}

    {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

    <section class="m-sec" aria-labelledby="ms-out">
      <h2 id="ms-out">{terminal ? t("phone.session.terminal") : t("phone.session.events")}</h2>
      <pre class="tail">{lines || t("phone.session.noOutput")}</pre>
    </section>
  {/if}
</main>

{#if s}
  <div class="dock" id="phone-session-dock" bind:offsetHeight={dockHeight}>
    {#if answerable}
      <div>
        <button class="btn accent" type="button" disabled={busy} onclick={() => answer(true)}><Check size={16} aria-hidden="true" />{t("phone.approve")}</button>
        <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(false)}><X size={16} aria-hidden="true" />{t("phone.deny")}</button>
      </div>
    {:else if terminal && live}
      <form class="stack" onsubmit={send}>
        <div class="keys" role="group" aria-label={t("phone.session.keys")}>
          {#each KEYS as k (k.key)}
            <button class="btn secondary" type="button" aria-label={k.name} onclick={() => press(k.key)}>{k.label}</button>
          {/each}
        </div>
        <div class="send-row">
          <label class="sr-only" for="ms-input">{t("phone.session.typeLabel")}</label>
          <input class="input" id="ms-input" bind:value={text} enterkeyhint="send" autocomplete="off" autocapitalize="off" placeholder={t("phone.session.typePlaceholder")} />
          <button class="btn primary" type="submit" disabled={sending || !text.trim()}><PaperPlaneTilt size={16} aria-hidden="true" />{t("phone.send")}</button>
        </div>
        {#if sendError}<p class="err-text small" role="alert">{tb(sendError)}</p>{/if}
      </form>
    {:else if terminal}
      <div class="stack">
        <button class="btn primary" type="button" id="btn-resume-session" disabled={busy} onclick={resume}><Play size={16} aria-hidden="true" />{busy ? t("phone.session.resuming") : t("phone.session.resume")}</button>
        {#if sendError}<p class="err-text small" role="alert">{tb(sendError)}</p>{:else}<p class="small">{t("phone.session.resumeHelp")}</p>{/if}
      </div>
    {:else}
      <form class="stack" onsubmit={send}>
        <div class="send-row">
          <label class="sr-only" for="ms-input">{t("phone.session.messageFor", { cli: CLI_LABEL[s.cli] })}</label>
          <textarea class="textarea" id="ms-input" rows="1" bind:value={text} disabled={!canFollowUp} placeholder={canFollowUp ? t("phone.session.followUpFor", { cli: CLI_LABEL[s.cli] }) : t("phone.session.unavailable")}></textarea>
          <button class="btn primary" type="submit" disabled={!canFollowUp || sending || !text.trim()}><PaperPlaneTilt size={16} aria-hidden="true" />{t("phone.send")}</button>
        </div>
        {#if sendError}<p class="err-text small" role="alert">{tb(sendError)}</p>{:else}<p class="small">{followUpHint}</p>{/if}
      </form>
    {/if}
  </div>
{/if}

<Dialog bind:open={stopOpen} labelledby="stop-sheet-title">
  {#if s}
    <div class="d-body" style="gap:14px">
      <h2 id="stop-sheet-title" style="font-size:20px">{t("phone.session.stopTitle")}</h2>
      <p class="m-p">{t("phone.session.stopBody", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })}</p>
      <div class="d-foot sheet-foot">
        <button class="btn danger" type="button" onclick={stop}><Stop size={16} aria-hidden="true" />{t("phone.session.stopConfirm")}</button>
        <button class="btn secondary" type="button" onclick={() => (stopOpen = false)}>{t("phone.session.keepRunning")}</button>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .sheet-foot {
    flex-direction: column;
    align-items: stretch;
  }
</style>
