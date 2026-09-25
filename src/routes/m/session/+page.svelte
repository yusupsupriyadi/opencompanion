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
  import { answer as sendAnswer, call, notify, onMessage, phone } from "$lib/phone.svelte";

  const id = $derived(page.url.searchParams.get("id") ?? "");

  // The keys a terminal UI asks for: confirm, cancel, move through a list, interrupt.
  const KEYS = [
    { key: "enter", label: "Enter", name: "Press Enter" },
    { key: "esc", label: "Esc", name: "Press Escape" },
    { key: "up", label: "↑", name: "Arrow up" },
    { key: "down", label: "↓", name: "Arrow down" },
    { key: "ctrl_c", label: "Ctrl+C", name: "Press Ctrl+C to interrupt" },
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
    const t = setInterval(() => {
      if (s && isLive(s)) load();
    }, 2500);
    return () => {
      off();
      clearInterval(t);
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
    if (s.status === "waiting") return "Answer the permission request above first.";
    if (live && s.cli !== "claude") return `${CLI_LABEL[s.cli]} is working. Send the next message when this turn finishes.`;
    if (!live && !s.cliSessionId) return `${CLI_LABEL[s.cli]} did not report a session id, so this conversation cannot continue.`;
    return `Starts the next turn of ${CLI_LABEL[s.cli]} in ${folderName(s.cwd)}.`;
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
            return `  edited ${ev.path}`;
          case "permission_request":
            return `? ${ev.tool}: ${ev.summary}`;
          case "permission_denied":
            return `✗ ${ev.tool} refused`;
          case "tool_failed":
            return `✗ ${ev.tool} failed: ${ev.message}`;
          case "error":
            return ev.message;
          case "done":
            return ev.ok ? `✓ Finished at ${clock(e.at)}` : `✗ Failed: ${ev.summary}`;
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
      notify(`Resumed ${CLI_LABEL[s.cli]} in ${folderName(s.cwd)}.`);
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

<svelte:head><title>{s ? `${s.title} · OpenCompanion` : "Session · OpenCompanion"}</title></svelte:head>

<header class="bar">
  <a class="back grow" href="/m"><CaretLeft size={20} aria-hidden="true" />Sessions</a>
  {#if s && live}
    <button class="btn danger" type="button" id="btn-stop-session" disabled={busy} onclick={() => (stopOpen = true)}><Stop size={16} aria-hidden="true" />Stop</button>
  {/if}
</header>
<main class="content" id="phone-session" style:padding-bottom={s ? `${dockHeight + 24}px` : undefined}>
  {#if loadState === "loading" && !s}
    <p class="m-p" role="status">Loading…</p>
  {:else if loadState === "missing" || !s}
    <h1 class="m-h1">Session not found</h1>
    <p class="m-p">It may have been removed from the history on your computer.</p>
    <a class="btn secondary block" href="/m">Back to sessions</a>
  {:else}
    <div class="row" style="align-items:flex-start">
      <CliMark kind={s.cli} />
      <h1 class="m-title grow">{s.title}</h1>
    </div>
    <div class="row" style="gap:8px;flex-wrap:wrap">
      <StatusChip status={s.status} />
      <span class="meta">{CLI_LABEL[s.cli]} · {s.mode} · started {ago(s.startedAt)}</span>
    </div>
    <p class="meta mono" style="margin:0;overflow-wrap:anywhere">{shortPath(s.cwd)}</p>

    {#if s.status === "waiting"}
      <section class="m-needs" aria-labelledby="ms-needs">
        <h2 id="ms-needs">{waitingTitle(s)}</h2>
        {#if s.waiting?.detail}<div class="cmd"><small>{s.waiting.tool ?? "Request"}</small><code>{s.waiting.detail}</code></div>{/if}
        {#if !s.waiting?.canAnswer}
          <p class="small" style="margin:0">
            {terminal ? "Answer it with the keys below, the same way you would in the terminal." : "Answer this in the session's terminal on your computer."}
          </p>
        {/if}
      </section>
    {/if}

    {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

    <section class="m-sec" aria-labelledby="ms-out">
      <h2 id="ms-out">{terminal ? "Terminal" : "Latest events"}</h2>
      <pre class="tail">{lines || "No output yet."}</pre>
    </section>
  {/if}
</main>

{#if s}
  <div class="dock" id="phone-session-dock" bind:offsetHeight={dockHeight}>
    {#if answerable}
      <div>
        <button class="btn accent" type="button" disabled={busy} onclick={() => answer(true)}><Check size={16} aria-hidden="true" />Approve</button>
        <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(false)}><X size={16} aria-hidden="true" />Deny</button>
      </div>
    {:else if terminal && live}
      <form class="stack" onsubmit={send}>
        <div class="keys" role="group" aria-label="Terminal keys">
          {#each KEYS as k (k.key)}
            <button class="btn secondary" type="button" aria-label={k.name} onclick={() => press(k.key)}>{k.label}</button>
          {/each}
        </div>
        <div class="send-row">
          <label class="sr-only" for="ms-input">Type into the terminal</label>
          <input class="input" id="ms-input" bind:value={text} enterkeyhint="send" autocomplete="off" autocapitalize="off" placeholder="Type, then Send to press Enter" />
          <button class="btn primary" type="submit" disabled={sending || !text.trim()}><PaperPlaneTilt size={16} aria-hidden="true" />Send</button>
        </div>
        {#if sendError}<p class="err-text small" role="alert">{sendError}</p>{/if}
      </form>
    {:else if terminal}
      <div class="stack">
        <button class="btn primary" type="button" id="btn-resume-session" disabled={busy} onclick={resume}><Play size={16} aria-hidden="true" />{busy ? "Resuming…" : "Resume terminal"}</button>
        {#if sendError}<p class="err-text small" role="alert">{sendError}</p>{:else}<p class="small">Opens the terminal on your computer again, with the CLI's own history when it has one.</p>{/if}
      </div>
    {:else}
      <form class="stack" onsubmit={send}>
        <div class="send-row">
          <label class="sr-only" for="ms-input">Message for {CLI_LABEL[s.cli]}</label>
          <textarea class="textarea" id="ms-input" rows="1" bind:value={text} disabled={!canFollowUp} placeholder={canFollowUp ? `Follow-up for ${CLI_LABEL[s.cli]}` : "Not available right now"}></textarea>
          <button class="btn primary" type="submit" disabled={!canFollowUp || sending || !text.trim()}><PaperPlaneTilt size={16} aria-hidden="true" />Send</button>
        </div>
        {#if sendError}<p class="err-text small" role="alert">{sendError}</p>{:else}<p class="small">{followUpHint}</p>{/if}
      </form>
    {/if}
  </div>
{/if}

<Dialog bind:open={stopOpen} labelledby="stop-sheet-title">
  {#if s}
    <div class="d-body" style="gap:14px">
      <h2 id="stop-sheet-title" style="font-size:20px">Stop this session?</h2>
      <p class="m-p">OpenCompanion sends {CLI_LABEL[s.cli]} in {folderName(s.cwd)} an interrupt first, then stops it by force after 3 seconds.</p>
      <div class="d-foot sheet-foot">
        <button class="btn danger" type="button" onclick={stop}><Stop size={16} aria-hidden="true" />Stop session</button>
        <button class="btn secondary" type="button" onclick={() => (stopOpen = false)}>Keep running</button>
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
