<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Plus from "phosphor-svelte/lib/Plus";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Trash from "phosphor-svelte/lib/Trash";
  import { onMount, tick, untrack } from "svelte";
  import { api, errorText, type ChatMessage, type ChatThread } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import DispatchCard from "$lib/DispatchCard.svelte";
  import FolderMention from "$lib/FolderMention.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import PlannerModel from "$lib/PlannerModel.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, isLive } from "$lib/format";
  import { app, showToast } from "$lib/store.svelte";

  let threads = $state<ChatThread[]>([]);
  let threadsState = $state<"loading" | "ready" | "error">("loading");
  let threadsError = $state("");
  let listOpen = $state(false);

  // The thread whose messages are on screen; `null` is a new chat that has no thread yet.
  let shownId = $state<string | null | undefined>(undefined);
  let messages = $state<ChatMessage[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let draft = $state("");
  // One planner turn at a time. It stays with its thread when the owner opens another chat.
  let pending = $state<{ threadId: string | null; text: string } | null>(null);
  let sendError = $state("");
  let confirmDelete = $state(false);
  let thread: HTMLDivElement | undefined = $state();
  let input: HTMLTextAreaElement | undefined = $state();
  let mention: FolderMention | undefined = $state();

  const currentId = $derived(page.url.searchParams.get("id"));
  const current = $derived(threads.find((t) => t.id === currentId));
  const missing = $derived(currentId !== null && threadsState === "ready" && !current);
  const thinkingHere = $derived(pending !== null && pending.threadId === currentId);

  const planner = $derived.by(() => {
    const chosen = app.settings?.chatCli;
    const usable = app.clis.filter((c) => c.path && c.kind !== "gemini");
    return usable.find((c) => c.kind === chosen) ?? usable[0] ?? null;
  });
  // A custom provider plans instead of a CLI once it has an address and a model.
  const provider = $derived.by(() => {
    const s = app.settings;
    if (s?.plannerSource !== "api") return null;
    const url = s.plannerApi?.baseUrl.trim() ?? "";
    const model = s.plannerApi?.model.trim() ?? "";
    let host = url;
    try {
      host = new URL(url).host;
    } catch {
      // Shown as typed; the backend explains what is wrong with it.
    }
    return { model, host, ready: Boolean(model && /^https?:\/\//i.test(url)) };
  });
  const canPlan = $derived(provider ? provider.ready : planner !== null);
  const liveSessions = $derived(app.sessions.filter(isLive));

  async function scrollDown() {
    await tick();
    if (thread) thread.scrollTop = thread.scrollHeight;
  }

  async function loadThreads() {
    threadsState = "loading";
    try {
      threads = await api.chatThreads();
      threadsState = "ready";
    } catch (e) {
      threadsState = "error";
      threadsError = errorText(e);
    }
  }

  async function open(id: string | null) {
    shownId = id;
    confirmDelete = false;
    sendError = "";
    if (id === null) {
      messages = [];
      loadState = "ready";
      return;
    }
    loadState = "loading";
    try {
      const got = await api.chatHistory(id);
      if (shownId !== id) return;
      // The backend stores the question before the planner answers; the pending bubble shows it already.
      const last = got.at(-1);
      const asked = pending?.threadId === id && last?.role === "user" && last.text === pending.text;
      messages = asked ? got.slice(0, -1) : got;
      loadState = "ready";
      scrollDown();
    } catch (e) {
      if (shownId !== id) return;
      loadState = "error";
      loadError = errorText(e);
    }
  }

  onMount(loadThreads);

  $effect(() => {
    const id = currentId;
    untrack(() => {
      if (id !== shownId) open(id);
    });
  });

  function keep(t: ChatThread) {
    threads = [t, ...threads.filter((x) => x.id !== t.id)];
  }

  async function newChat() {
    listOpen = false;
    await goto("/chat");
    input?.focus();
  }

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text || pending) return;
    const threadId = currentId;
    pending = { threadId, text };
    sendError = "";
    draft = "";
    scrollDown();
    try {
      const turn = await api.chatSend(threadId, text);
      keep(turn.thread);
      if (shownId === threadId) {
        messages = [...messages.filter((m) => m.id !== turn.user.id), turn.user, turn.reply];
        if (threadId === null) {
          shownId = turn.thread.id;
          await goto(`/chat?id=${turn.thread.id}`, { replaceState: true, keepFocus: true, noScroll: true });
        }
      }
    } catch (err) {
      if (shownId === threadId) {
        sendError = errorText(err);
        draft = text;
      } else {
        showToast(`The planner could not answer in the other chat: ${errorText(err)}`);
      }
    } finally {
      pending = null;
      scrollDown();
      input?.focus();
    }
  }

  async function deleteChat() {
    if (!current) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    const gone = current;
    try {
      await api.chatDeleteThread(gone.id);
      threads = threads.filter((t) => t.id !== gone.id);
      showToast(`Deleted "${gone.title}".`);
      await goto("/chat");
    } catch (err) {
      confirmDelete = false;
      showToast(`The chat could not be deleted: ${errorText(err)}`);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (mention?.keydown(e)) return;
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }

  function replace(m: ChatMessage) {
    messages = messages.map((x) => (x.id === m.id ? m : x));
  }
</script>

<svelte:head><title>{current ? `${current.title} · ` : ""}Chat · OpenCompanion</title></svelte:head>

<div class="chat-wrap">
  <nav class="history" class:open={listOpen} id="chat-history" aria-labelledby="history-title">
    <div class="history-head">
      <h2 id="history-title" class="grow">Chats</h2>
      <button class="btn secondary sm" type="button" id="btn-new-chat" onclick={newChat}>
        <Plus size={16} aria-hidden="true" />New chat
      </button>
    </div>
    <button class="history-toggle" type="button" aria-expanded={listOpen} aria-controls="chat-history-list" onclick={() => (listOpen = !listOpen)}>
      <span class="grow ellipsis">{current?.title ?? "New chat"}</span>
      {#if threadsState === "ready"}<span class="meta">{threads.length} saved</span>{/if}
      <CaretDown size={16} aria-hidden="true" />
    </button>
    <div class="history-list" id="chat-history-list">
      {#if threadsState === "loading"}
        <p class="meta" role="status">Loading your chats…</p>
      {:else if threadsState === "error"}
        <p class="err-text" role="alert">The chat list could not be loaded: {threadsError}</p>
        <button class="btn secondary sm" type="button" onclick={loadThreads}>Try again</button>
      {:else if threads.length === 0}
        <p class="meta">No chats yet. Your first message starts one, and every chat stays here with its own history.</p>
      {:else}
        {#each threads as t (t.id)}
          <a
            class="chat-item"
            href="/chat?id={t.id}"
            aria-current={t.id === currentId ? "page" : undefined}
            title={t.title}
            onclick={() => (listOpen = false)}
          >
            <b class="ellipsis">{t.title}</b>
            <span>{pending?.threadId === t.id ? "Planner is answering…" : ago(t.updatedAt, app.now)}</span>
          </a>
        {/each}
      {/if}
    </div>
  </nav>

  <main class="chat-col" id="chat-main">
    <header class="page-head">
      <div class="grow"><h1 class="sr-only">Chat</h1></div>
      {#if current}
        <button class="btn ghost" type="button" id="btn-delete-chat" onclick={deleteChat} disabled={thinkingHere}>
          <Trash size={16} aria-hidden="true" />{confirmDelete ? "Press again to delete" : "Delete chat"}
        </button>
      {/if}
      <a class="btn secondary" href="/settings#chat">Change planner</a>
    </header>

    <div class="thread" bind:this={thread} aria-live="polite">
      {#if missing}
        <div class="state-box" role="alert">
          <h2>This chat is gone</h2>
          <p>It was deleted, so its messages are no longer stored. Sessions started from its cards keep running.</p>
          <button class="btn secondary" type="button" onclick={newChat}>Start a new chat</button>
        </div>
      {:else if loadState === "loading"}
        <p class="hint" role="status">Loading the conversation…</p>
      {:else if loadState === "error"}
        <div class="state-box" role="alert">
          <h2>The conversation could not be loaded</h2>
          <p>{loadError}</p>
          <button class="btn secondary" type="button" onclick={() => open(currentId)}>Try again</button>
        </div>
      {/if}

      {#if !missing && loadState === "ready"}
        {#each messages as m (m.id)}
          {#if m.role === "user"}
            <p class="me">{m.text}</p>
          {:else}
            <div class="planner">
              <span class="who">Planner</span>
              {#if m.text}<p class:err-text={m.role === "error"}>{m.text}</p>{/if}
              {#each m.cards as c (c.id)}
                <DispatchCard card={c} messageId={m.id} onchange={replace} />
              {/each}
            </div>
          {/if}
        {/each}
      {/if}

      {#if thinkingHere && pending}
        <p class="me">{pending.text}</p>
        <div class="planner">
          <span class="who">Planner</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{#if provider}{provider.model || "The custom provider"} is reading your message.{:else}{planner?.label ?? "The planner"} is reading your message. This usually takes 10 to 30 seconds.{/if}</p>
        </div>
      {/if}
    </div>

    <form class="composer" onsubmit={send}>
      <label class="sr-only" for="composer-input">Message the planner</label>
      <FolderMention bind:this={mention} textarea={input} />
      <textarea
        id="composer-input"
        rows="2"
        bind:this={input}
        bind:value={draft}
        {onkeydown}
        disabled={missing}
        placeholder="Describe a task. Name a CLI, type @ for a folder, or let the planner pick one."
      ></textarea>
      {#if sendError}<p class="err-text" role="alert" style="margin:0">{sendError}</p>{/if}
      <div class="composer-bar">
        {#if planner && app.settings?.plannerSource !== "api"}<PlannerModel cli={planner} />{/if}
        <button
          class="btn primary send"
          type="submit"
          id="btn-send"
          aria-label="Send"
          title="Send. Shift+Enter adds a line."
          disabled={pending !== null || !draft.trim() || !canPlan || missing}
        >
          <PaperPlaneTilt size={16} weight="fill" aria-hidden="true" />
        </button>
      </div>
      {#if pending && !thinkingHere}
        <p class="meta composer-hint" role="status">The planner is still answering in another chat. Send works again when it is done.</p>
      {/if}
      {#if !canPlan}
        <p class="meta composer-hint" role="status">
          {#if provider}The custom provider has no base URL or model yet. Add them in Settings.
          {:else if app.clisState === "loading"}Checking which CLI can plan…
          {:else}No CLI that can plan is installed.{/if}
        </p>
      {/if}
    </form>
  </main>

  <aside class="rail" aria-labelledby="rail-title">
    <h2 id="rail-title">Live sessions</h2>
    {#if liveSessions.length === 0}
      <p class="meta" style="margin:0">Nothing is running. Sessions you start from a card show up here.</p>
    {/if}
    {#each liveSessions as s (s.id)}
      <a class="live" href="/session?id={s.id}">
        <span class="row" style="gap:8px">
          <CliMark kind={s.cli} small />
          <b class="grow" title={s.title}>{s.title}</b>
          <StatusChip status={s.status} />
        </span>
        <Horizon marks={s.marks} start={s.startedAt} end={app.now} live={s.status === "running"} full />
        <span class="meta" style="font-size:12px"><span title={s.cwd}>{CLI_LABEL[s.cli]} in {folderName(s.cwd)}</span> · {s.lastEvent ?? "Starting"}</span>
      </a>
    {/each}
  </aside>
</div>

<style>
  .chat-wrap {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr) 300px;
    gap: 12px;
    min-height: 0;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .history {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 20px 12px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    min-height: 0;
    overflow-y: auto;
  }
  .history-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 8px;
  }
  .history h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 800;
  }
  .history-toggle {
    display: none;
  }
  .history-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .history-list > p {
    margin: 0;
    padding: 0 8px;
  }
  .history-list > .btn {
    align-self: flex-start;
    margin: 8px 0 0 8px;
  }
  .chat-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 44px;
    padding: 7px 10px;
    border-radius: var(--r-btn);
    transition: background-color 0.12s ease-out;
  }
  .chat-item:hover,
  .chat-item[aria-current="page"] {
    background: var(--surface-2);
  }
  .chat-item b {
    font-size: 13px;
    font-weight: 700;
  }
  .chat-item[aria-current="page"] b {
    font-weight: 800;
  }
  .chat-item span {
    font-size: 12px;
    color: var(--ink-2);
  }
  .chat-col {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 24px 28px;
    min-width: 0;
    min-height: 0;
  }
  .thread {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-right: 4px;
  }
  .me {
    align-self: flex-end;
    max-width: 560px;
    padding: 12px 16px;
    border-radius: 14px 14px 4px 14px;
    background: var(--surface-2);
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .planner {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .planner > p {
    margin: 0;
    white-space: pre-wrap;
  }
  .who {
    font: 12px var(--font-pixel);
    color: var(--ink-2);
  }
  .thinking {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .composer {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line-strong);
  }
  /* The ring follows the text box; the pickers show their own. */
  .composer:has(textarea:focus-visible) {
    outline: 2px solid var(--forest);
    outline-offset: 2px;
  }
  .composer textarea {
    border: 0;
    background: transparent;
    resize: none;
    min-height: 44px;
    line-height: 1.45;
  }
  .composer textarea:focus-visible {
    outline: none;
  }
  .composer textarea::placeholder {
    color: var(--ink-2);
  }
  .composer-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .composer-bar :global(.pickers) {
    flex: 1 1 auto;
  }
  .composer-bar > .btn {
    margin-left: auto;
  }
  .composer-bar > .send {
    width: 36px;
    min-height: 36px;
    padding: 0;
  }
  .composer-hint {
    margin: 0;
    font-size: 12px;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 28px 20px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    overflow-y: auto;
  }
  .rail h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 800;
  }
  .live {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: var(--r-md);
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .live:hover {
    border-color: var(--line-strong);
  }
  .live b {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @media (max-width: 1279px) {
    .chat-wrap {
      grid-template-columns: 240px minmax(0, 1fr);
    }
    .rail {
      display: none;
    }
  }
  @media (max-width: 900px) {
    .chat-wrap {
      grid-template-columns: 208px minmax(0, 1fr);
    }
    .chat-col {
      padding: 20px;
    }
  }
  /* Narrow window: the list folds into one row above the conversation. */
  @media (max-width: 720px) {
    .chat-wrap {
      grid-template-columns: minmax(0, 1fr);
    }
    .history {
      padding: 12px;
      overflow: visible;
    }
    .history-toggle {
      display: flex;
      align-items: center;
      gap: 10px;
      min-height: 44px;
      padding: 8px 10px;
      border: 1px solid var(--line-strong);
      border-radius: var(--r-btn);
      background: var(--surface);
      color: var(--ink);
      font-weight: 700;
      text-align: left;
      cursor: pointer;
    }
    .history-toggle[aria-expanded="true"] :global(svg) {
      transform: rotate(180deg);
    }
    .history:not(.open) .history-list {
      display: none;
    }
    .history-head .btn.sm {
      min-height: 44px;
    }
    .composer-bar > .send {
      width: 44px;
      min-height: 44px;
    }
    .chat-col {
      padding: 20px 16px;
    }
    .thread {
      overflow: visible;
    }
  }
</style>
