<script lang="ts">
  import Cloud from "phosphor-svelte/lib/Cloud";
  import Plus from "phosphor-svelte/lib/Plus";
  import { onMount } from "svelte";
  import type { ChatThread } from "$lib/api";
  import { ago } from "$lib/format";
  import { t } from "$lib/i18n.svelte";
  import { call, onMessage } from "$lib/phone.svelte";

  let threads = $state<ChatThread[]>([]);
  let planner = $state<string | null>(null);
  let answering = $state<string[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let now = $state(Date.now());

  async function load(quiet = false) {
    if (!quiet) loadState = "loading";
    try {
      const r = await call<{ threads: ChatThread[]; planner: string | null; answering?: string[] }>("/api/chat");
      threads = r.threads;
      planner = r.planner;
      answering = r.answering ?? [];
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
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      off();
      clearInterval(timer);
    };
  });
</script>

<svelte:head><title>{t("phone.nav.chat")} · OpenCompanion</title></svelte:head>

<header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-chat-list">
  <div class="head-row">
    <h1 class="m-h1">{t("phone.nav.chat")}</h1>
    <a class="btn secondary" href="/m/chat/thread" id="btn-new-chat"><Plus size={16} aria-hidden="true" />{t("phone.newChat")}</a>
  </div>

  {#if loadState === "loading"}
    <p class="m-p" role="status">{t("phone.chat.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{t("phone.chat.loadFailed", { error: loadError })}</p>
    <button class="btn secondary block" type="button" onclick={() => load()}>{t("phone.tryAgain")}</button>
  {:else}
    <p class="m-p">
      {#if planner}{t("phone.chat.planner", { planner })}
      {:else}{t("phone.chat.noPlanner")}{/if}
    </p>
    {#if threads.length === 0}
      <p class="m-p">{t("phone.chat.empty")}</p>
    {:else}
      <nav class="m-list" aria-label={t("phone.chat.saved")}>
        {#each threads as thread (thread.id)}
          <a href="/m/chat/thread?id={thread.id}">
            <b>{thread.title}</b>
            <span class="meta">{answering.includes(thread.id) ? t("phone.chat.answering") : ago(thread.updatedAt, now)}</span>
          </a>
        {/each}
      </nav>
    {/if}
  {/if}
</main>
