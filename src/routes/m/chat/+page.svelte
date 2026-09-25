<script lang="ts">
  import Cloud from "phosphor-svelte/lib/Cloud";
  import Plus from "phosphor-svelte/lib/Plus";
  import { onMount } from "svelte";
  import type { ChatThread } from "$lib/api";
  import { ago } from "$lib/format";
  import { call, onMessage } from "$lib/phone.svelte";

  let threads = $state<ChatThread[]>([]);
  let planner = $state<string | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let now = $state(Date.now());

  async function load(quiet = false) {
    if (!quiet) loadState = "loading";
    try {
      const r = await call<{ threads: ChatThread[]; planner: string | null }>("/api/chat");
      threads = r.threads;
      planner = r.planner;
      loadState = "ready";
    } catch (e) {
      if (quiet) return;
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    load();
    const off = onMessage((m) => {
      if (m.type === "chat") load(true);
    });
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      off();
      clearInterval(t);
    };
  });
</script>

<svelte:head><title>Chat · OpenCompanion</title></svelte:head>

<header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-chat-list">
  <div class="head-row">
    <h1 class="m-h1">Chat</h1>
    <a class="btn secondary" href="/m/chat/thread" id="btn-new-chat"><Plus size={16} aria-hidden="true" />New chat</a>
  </div>

  {#if loadState === "loading"}
    <p class="m-p" role="status">Loading your chats…</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">The chat list could not be loaded: {loadError}</p>
    <button class="btn secondary block" type="button" onclick={() => load()}>Try again</button>
  {:else}
    <p class="m-p">
      {#if planner}Planner: {planner}. It answers with a card for each session, and nothing starts until you press Run.
      {:else}Nothing can plan yet. Install Claude Code, Codex CLI or OpenCode on your computer, or choose a planner in its Settings.{/if}
    </p>
    {#if threads.length === 0}
      <p class="m-p">No chats yet. Your first message starts one, and every chat keeps its own history on your computer.</p>
    {:else}
      <nav class="m-list" aria-label="Saved chats">
        {#each threads as t (t.id)}
          <a href="/m/chat/thread?id={t.id}">
            <b>{t.title}</b>
            <span class="meta">{ago(t.updatedAt, now)}</span>
          </a>
        {/each}
      </nav>
    {/if}
  {/if}
</main>
