<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import { onMount, tick, untrack } from "svelte";
  import type { ChatMessage, ChatThread } from "$lib/api";
  import PhoneDispatchCard from "$lib/PhoneDispatchCard.svelte";
  import { folderName } from "$lib/format";
  import { plural, t, tb } from "$lib/i18n.svelte";
  import { call, onMessage, PhoneError } from "$lib/phone.svelte";

  // No `id` is a new chat: the first message creates its thread.
  const id = $derived(page.url.searchParams.get("id"));

  let thread = $state<ChatThread | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let loadState = $state<"loading" | "ready" | "missing" | "error">("loading");
  let loadError = $state("");
  let shownId = $state<string | null | undefined>(undefined);
  let draft = $state("");
  let pending = $state<string | null>(null);
  // The planner is answering a message sent before this page opened, or from the desktop.
  let answering = $state(false);
  // What a screen reader hears when an answer lands; the thread itself is not a live region.
  let announcement = $state("");
  // Folders whose cards start without Run; the note under the composer says so (PRD FR-26).
  let autoRun = $state<string[]>([]);
  let sendError = $state("");
  let dockHeight = $state(0);

  async function scrollDown() {
    await tick();
    window.scrollTo({ top: document.body.scrollHeight });
  }

  async function open(target: string | null, quiet = false) {
    shownId = target;
    if (!target) {
      thread = null;
      messages = [];
      loadState = "ready";
      return;
    }
    if (!quiet) loadState = "loading";
    try {
      const r = await call<{ thread: ChatThread; messages: ChatMessage[]; answering?: boolean; autoRun?: string[] }>(`/api/chat/${target}`);
      if (shownId !== target) return;
      const before = messages.length;
      thread = r.thread;
      messages = r.messages;
      answering = Boolean(r.answering);
      autoRun = r.autoRun ?? [];
      loadState = "ready";
      if (quiet && messages.length > before && messages.at(-1)?.role !== "user") announce(messages.at(-1)!);
      if (!quiet) scrollDown();
    } catch (e) {
      if (shownId !== target) return;
      if (e instanceof PhoneError && e.status === 404) loadState = "missing";
      else if (!quiet) {
        loadState = "error";
        loadError = e instanceof Error ? e.message : String(e);
      }
    }
  }

  $effect(() => {
    const target = id;
    untrack(() => {
      if (target !== shownId) open(target);
    });
  });

  onMount(() =>
    // A turn or a card changed elsewhere (the desktop, or another phone). The own turn merges itself.
    onMessage((m) => {
      if (m.type === "chat" && m.threadId && m.threadId === shownId && !pending) open(m.threadId, true);
    }),
  );

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text || pending) return;
    const threadId = id;
    pending = text;
    draft = "";
    sendError = "";
    scrollDown();
    try {
      const turn = await call<{ thread: ChatThread; user: ChatMessage; reply: ChatMessage }>("/api/chat", {
        method: "POST",
        body: JSON.stringify({ threadId, message: text }),
      });
      if (shownId === threadId) {
        thread = turn.thread;
        messages = [...messages.filter((m) => m.id !== turn.user.id && m.id !== turn.reply.id), turn.user, turn.reply];
        announce(turn.reply);
        if (threadId === null) {
          shownId = turn.thread.id;
          await goto(`/m/chat/thread?id=${turn.thread.id}`, { replaceState: true, keepFocus: true, noScroll: true });
        }
      }
    } catch (err) {
      sendError = err instanceof Error ? err.message : String(err);
      draft = text;
    } finally {
      pending = null;
      scrollDown();
    }
  }

  function announce(m: ChatMessage) {
    const n = m.cards.length;
    announcement = m.role === "error" ? m.text : n ? plural(n, "phone.thread.answeredOne", "phone.thread.answeredMany") : t("phone.thread.answered");
  }

  function replace(m: ChatMessage) {
    messages = messages.map((x) => (x.id === m.id ? m : x));
  }
</script>

<svelte:head><title>{thread ? `${thread.title} · ` : ""}{t("phone.nav.chat")} · OpenCompanion</title></svelte:head>

<header class="bar">
  <a class="back" href="/m/chat"><CaretLeft size={20} aria-hidden="true" />{t("phone.thread.back")}</a>
</header>
<main class="content" id="phone-chat-thread" style:padding-bottom="{dockHeight + 24}px">
  <h1 class="m-title">{thread?.title ?? t("phone.newChat")}</h1>

  {#if loadState === "missing"}
    <p class="m-p" role="alert">{t("phone.thread.deleted")}</p>
    <a class="btn secondary block" href="/m/chat/thread">{t("phone.thread.startNew")}</a>
  {:else if loadState === "loading"}
    <p class="m-p" role="status">{t("phone.thread.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{t("phone.thread.loadFailed", { error: loadError })}</p>
    <button class="btn secondary block" type="button" onclick={() => open(id)}>{t("phone.tryAgain")}</button>
  {:else}
    <p class="sr-only" role="status">{announcement}</p>
    <div class="m-thread">
      {#if messages.length === 0 && !pending}
        <div class="m-planner">
          <span class="who">{t("phone.thread.planner")}</span>
          <p>{t("phone.thread.intro")}</p>
        </div>
      {/if}
      {#each messages as m (m.id)}
        {#if m.role === "user"}
          <p class="m-me">{m.text}</p>
        {:else}
          <div class="m-planner">
            <span class="who">{t("phone.thread.planner")}</span>
            {#if m.text}<p class:err-text={m.role === "error"}>{tb(m.text)}</p>{/if}
            {#each m.cards as c (c.id)}
              <PhoneDispatchCard card={c} messageId={m.id} onchange={replace} />
            {/each}
          </div>
        {/if}
      {/each}
      {#if pending}
        <p class="m-me">{pending}</p>
        <div class="m-planner">
          <span class="who">{t("phone.thread.planner")}</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{t("phone.thread.reading")}</p>
        </div>
      {:else if answering}
        <div class="m-planner">
          <span class="who">{t("phone.thread.planner")}</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{t("phone.thread.stillAnswering")}</p>
        </div>
      {/if}
    </div>
  {/if}
</main>

{#if loadState === "ready"}
  <div class="dock" id="phone-chat-composer" bind:offsetHeight={dockHeight}>
    <form class="stack" onsubmit={send}>
      <div class="send-row">
        <label class="sr-only" for="pc-input">{t("phone.thread.inputLabel")}</label>
        <textarea class="textarea" id="pc-input" rows="2" bind:value={draft} placeholder={t("phone.thread.inputPlaceholder")}></textarea>
        <button class="btn primary" type="submit" id="btn-chat-send" disabled={pending !== null || answering || !draft.trim()}><PaperPlaneTilt size={16} aria-hidden="true" />{t("phone.send")}</button>
      </div>
      {#if sendError}<p class="err-text small" role="alert">{tb(sendError)}</p>
      {:else if autoRun.length}<p class="small">{t("phone.thread.autoRun", { folders: autoRun.map(folderName).join(", ") })}</p>
      {:else}<p class="small">{t("phone.thread.noAutoRun")}</p>{/if}
    </form>
  </div>
{/if}
