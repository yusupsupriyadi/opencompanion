<script lang="ts">
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import Check from "phosphor-svelte/lib/Check";
  import Stop from "phosphor-svelte/lib/Stop";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import type { EventRow, SessionInfo } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, clock, folderName, isLive, shortPath, waitingTitle } from "$lib/format";
  import { call, onMessage, phone } from "$lib/phone.svelte";

  const id = $derived(page.url.searchParams.get("id") ?? "");

  let loaded = $state<{ session: SessionInfo; events: EventRow[]; tail: string } | null>(null);
  let loadState = $state<"loading" | "ready" | "missing">("loading");
  let failure = $state("");
  let busy = $state(false);
  let stopOpen = $state(false);

  async function load() {
    try {
      loaded = await call(`/api/sessions/${id}`);
      loadState = "ready";
    } catch (e) {
      if (e instanceof Error && "status" in e && (e as { status: number }).status === 404) loadState = "missing";
      else failure = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    load();
    let pending: ReturnType<typeof setTimeout> | undefined;
    const off = onMessage((m) => {
      // Refresh the tail and events shortly after this session changes.
      const sid = m.session?.id ?? m.event?.sessionId;
      if (sid !== id) return;
      clearTimeout(pending);
      pending = setTimeout(load, 400);
    });
    const t = setInterval(() => {
      if (s && isLive(s)) load();
    }, 5000);
    return () => {
      off();
      clearInterval(t);
      clearTimeout(pending);
    };
  });

  const s = $derived(phone.sessions.find((x) => x.id === id) ?? loaded?.session ?? null);

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
      await call(`/api/sessions/${id}/answer`, { method: "POST", body: JSON.stringify({ allow }) });
      await load();
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

  const dock = $derived(Boolean(s && (s.status === "waiting" ? s.waiting?.canAnswer : isLive(s))));
</script>

<svelte:head><title>{s ? `${s.title} · OpenCompanion` : "Session · OpenCompanion"}</title></svelte:head>

<header class="bar">
  <a class="back" href="/m"><CaretLeft size={20} aria-hidden="true" />Sessions</a>
</header>
<main class="content" class:has-dock={dock} id="phone-session">
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
        {#if !s.waiting?.canAnswer}<p class="small" style="margin:0">Answer this in the session's terminal on your computer.</p>{/if}
      </section>
    {/if}

    {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

    <section class="m-sec" aria-labelledby="ms-out">
      <h2 id="ms-out">{s.mode === "interactive" ? "Terminal" : "Latest events"}</h2>
      <pre class="tail">{lines || "No output yet."}</pre>
    </section>
  {/if}
</main>

{#if s && dock}
  <div class="dock">
    {#if s.status === "waiting" && s.waiting?.canAnswer}
      <div>
        <button class="btn accent" type="button" disabled={busy} onclick={() => answer(true)}><Check size={16} aria-hidden="true" />Approve</button>
        <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(false)}><X size={16} aria-hidden="true" />Deny</button>
      </div>
    {:else}
      <div class="one">
        <button class="btn danger" type="button" disabled={busy} onclick={() => (stopOpen = true)}><Stop size={16} aria-hidden="true" />Stop session</button>
      </div>
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
