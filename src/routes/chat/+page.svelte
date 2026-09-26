<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Plus from "phosphor-svelte/lib/Plus";
  import SidebarSimple from "phosphor-svelte/lib/SidebarSimple";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Trash from "phosphor-svelte/lib/Trash";
  import { onMount, tick, untrack } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { api, errorText, type ChatMessage, type ChatThread } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import DispatchCard from "$lib/DispatchCard.svelte";
  import FolderMention from "$lib/FolderMention.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import PlannerModel from "$lib/PlannerModel.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, isLive } from "$lib/format";
  import { plural, t, tb } from "$lib/i18n.svelte";
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
  // Threads the planner is answering in, as the backend knows them: a turn sent before this page
  // was opened, or one sent from the phone.
  let answering = $state<string[]>([]);
  // What a screen reader hears when an answer lands; the thread itself is not a live region.
  let announcement = $state("");
  let confirmDelete = $state(false);
  let thread: HTMLDivElement | undefined = $state();
  let input: HTMLTextAreaElement | undefined = $state();
  let mention: FolderMention | undefined = $state();

  const currentId = $derived(page.url.searchParams.get("id"));
  const current = $derived(threads.find((t) => t.id === currentId));
  const missing = $derived(currentId !== null && threadsState === "ready" && !current);
  const thinkingHere = $derived(pending !== null && pending.threadId === currentId);
  const answeringHere = $derived(!thinkingHere && currentId !== null && answering.includes(currentId));

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
  // PRD FR-26 asks for a plain sign whenever cards can start without Run.
  const autoRun = $derived(app.settings?.autoRunFolders ?? []);
  const liveSessions = $derived(app.sessions.filter(isLive));

  // A wide window docks the rail and remembers when it was hidden; a narrower one opens it over the conversation.
  const RAIL_KEY = "air-chat-rail-hidden";
  const wide = new MediaQuery("min-width: 1280px", true);
  let railHidden = $state(readRailHidden());
  let railPeek = $state(false);
  let railBtn: HTMLButtonElement | undefined = $state();
  let railPop: HTMLElement | undefined = $state();
  const railOpen = $derived(wide.current ? !railHidden : railPeek);

  function readRailHidden() {
    try {
      return localStorage.getItem(RAIL_KEY) === "1";
    } catch {
      return false;
    }
  }

  function toggleRail() {
    if (!wide.current) {
      railPeek = !railPeek;
      return;
    }
    railHidden = !railHidden;
    try {
      if (railHidden) localStorage.setItem(RAIL_KEY, "1");
      else localStorage.removeItem(RAIL_KEY);
    } catch {
      // Private windows can refuse storage; the change still holds for this session.
    }
  }

  // Escape closes the floating rail from anywhere, unless another control used it first (the @ folder list).
  function closePeek(e: KeyboardEvent) {
    if (e.key !== "Escape" || !railPeek || e.defaultPrevented) return;
    const inside = railPop?.contains(document.activeElement);
    railPeek = false;
    if (inside) railBtn?.focus();
  }

  $effect(() => {
    if (wide.current) railPeek = false;
  });

  async function scrollDown() {
    await tick();
    if (thread) thread.scrollTop = thread.scrollHeight;
  }

  async function loadThreads(quiet = false) {
    if (!quiet) threadsState = "loading";
    try {
      threads = await api.chatThreads();
      threadsState = "ready";
    } catch (e) {
      if (quiet) return;
      threadsState = "error";
      threadsError = errorText(e);
    }
  }

  async function loadAnswering() {
    answering = (await api.chatAnswering().catch(() => answering)) ?? [];
  }

  /** `quiet` reloads the thread on screen without the loading state, after a change elsewhere. */
  async function open(id: string | null, quiet = false) {
    if (!quiet) {
      shownId = id;
      confirmDelete = false;
      sendError = "";
    }
    if (id === null) {
      messages = [];
      loadState = "ready";
      return;
    }
    if (!quiet) loadState = "loading";
    try {
      const got = await api.chatHistory(id);
      if (shownId !== id) return;
      // The backend stores the question before the planner answers; the pending bubble shows it already.
      const last = got.at(-1);
      const asked = pending?.threadId === id && last?.role === "user" && last.text === pending.text;
      const before = messages.length;
      messages = asked ? got.slice(0, -1) : got;
      loadState = "ready";
      if (quiet && messages.length > before && messages.at(-1)?.role !== "user") announce(messages.at(-1)!);
      scrollDown();
    } catch (e) {
      if (shownId !== id || quiet) return;
      loadState = "error";
      loadError = errorText(e);
    }
  }

  function announce(m: ChatMessage) {
    const n = m.cards.length;
    announcement = m.role === "error" ? m.text : n ? plural(n, "chat.announce.cardsOne", "chat.announce.cardsMany") : t("chat.announce.answered");
  }

  onMount(() => {
    loadThreads();
    loadAnswering();
    // A turn started, finished or a card changed: in this window, in another chat, or on the phone.
    // The turn this page is waiting for merges its own answer, so only other changes reload.
    const un = listen<string>("chat-changed", (e) => {
      loadThreads(true);
      loadAnswering();
      if (e.payload === shownId && pending?.threadId !== shownId) open(shownId, true);
    });
    return () => {
      un.then((f) => f());
    };
  });

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
        messages = [...messages.filter((m) => m.id !== turn.user.id && m.id !== turn.reply.id), turn.user, turn.reply];
        announce(turn.reply);
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
        showToast(t("chat.thread.otherChatFailed", { error: errorText(err) }));
      }
    } finally {
      pending = null;
      scrollDown();
      // Back to the composer only when focus is still there; the owner may be busy elsewhere by now.
      const at = document.activeElement;
      if (!at || at === document.body || at === input) input?.focus();
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
      showToast(t("chat.delete.done", { title: gone.title }));
      await goto("/chat");
    } catch (err) {
      confirmDelete = false;
      showToast(t("chat.delete.failed", { error: errorText(err) }));
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

<svelte:head><title>{current ? `${current.title} · ` : ""}{t("chat.heading")} · OpenCompanion</title></svelte:head>
<svelte:window onkeydown={closePeek} />

<div class="chat-wrap" class:docked={railOpen && wide.current}>
  <nav class="history" class:open={listOpen} id="chat-history" aria-labelledby="history-title">
    <div class="history-head">
      <h2 id="history-title" class="grow">{t("chat.history.title")}</h2>
      <button class="btn secondary sm" type="button" id="btn-new-chat" onclick={newChat}>
        <Plus size={16} aria-hidden="true" />{t("chat.history.new")}
      </button>
    </div>
    <button class="history-toggle" type="button" aria-expanded={listOpen} aria-controls="chat-history-list" onclick={() => (listOpen = !listOpen)}>
      <span class="grow ellipsis">{current?.title ?? t("chat.history.new")}</span>
      {#if threadsState === "ready"}<span class="meta">{t("chat.history.saved", { n: threads.length })}</span>{/if}
      <CaretDown size={16} aria-hidden="true" />
    </button>
    <div class="history-list" id="chat-history-list">
      {#if threadsState === "loading"}
        <p class="meta" role="status">{t("chat.history.loading")}</p>
      {:else if threadsState === "error"}
        <p class="err-text" role="alert">{t("chat.history.loadFailed", { error: threadsError })}</p>
        <button class="btn secondary sm" type="button" onclick={() => loadThreads()}>{t("chat.tryAgain")}</button>
      {:else if threads.length === 0}
        <p class="meta">{t("chat.history.empty")}</p>
      {:else}
        {#each threads as chat (chat.id)}
          <a
            class="chat-item"
            href="/chat?id={chat.id}"
            aria-current={chat.id === currentId ? "page" : undefined}
            title={chat.title}
            onclick={() => (listOpen = false)}
          >
            <b class="ellipsis">{chat.title}</b>
            <span>{pending?.threadId === chat.id || answering.includes(chat.id) ? t("chat.history.answering") : ago(chat.updatedAt, app.now)}</span>
          </a>
        {/each}
      {/if}
    </div>
  </nav>

  <main class="chat-col" id="chat-main">
    <header class="page-head">
      <h1 class="sr-only">{t("chat.heading")}</h1>
      {#if current}
        <button class="btn ghost" type="button" id="btn-delete-chat" onclick={deleteChat} disabled={thinkingHere || answeringHere}>
          <Trash size={16} aria-hidden="true" />{confirmDelete ? t("chat.delete.confirm") : t("chat.delete.button")}
        </button>
      {/if}
      <button
        class="icon-btn"
        type="button"
        id="btn-live-rail"
        bind:this={railBtn}
        aria-expanded={railOpen}
        aria-controls="chat-live-rail"
        aria-label={t("chat.live.title")}
        title={railOpen ? t("chat.live.hide") : t("chat.live.show")}
        onclick={toggleRail}
      >
        <SidebarSimple size={20} weight={railOpen ? "fill" : "regular"} mirrored aria-hidden="true" />
      </button>
      {#if railOpen && !wide.current}
        <aside class="rail pop" id="chat-live-rail" aria-labelledby="rail-title" bind:this={railPop}>
          {@render live()}
        </aside>
      {/if}
    </header>

    <p class="sr-only" role="status" id="chat-announcement">{announcement}</p>
    <div class="thread" bind:this={thread}>
      {#if missing}
        <div class="state-box" role="alert">
          <h2>{t("chat.gone.title")}</h2>
          <p>{t("chat.gone.body")}</p>
          <button class="btn secondary" type="button" onclick={newChat}>{t("chat.gone.start")}</button>
        </div>
      {:else if loadState === "loading"}
        <p class="hint" role="status">{t("chat.thread.loading")}</p>
      {:else if loadState === "error"}
        <div class="state-box" role="alert">
          <h2>{t("chat.thread.loadFailed")}</h2>
          <p>{tb(loadError)}</p>
          <button class="btn secondary" type="button" onclick={() => open(currentId)}>{t("chat.tryAgain")}</button>
        </div>
      {/if}

      {#if !missing && loadState === "ready"}
        {#each messages as m (m.id)}
          {#if m.role === "user"}
            <p class="me">{m.text}</p>
          {:else}
            <div class="planner">
              <span class="who">{t("chat.thread.planner")}</span>
              {#if m.text}<p class:err-text={m.role === "error"}>{tb(m.text)}</p>{/if}
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
          <span class="who">{t("chat.thread.planner")}</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{#if provider}{t("chat.thread.readingProvider", { name: provider.model || t("chat.thread.customProvider") })}{:else}{t("chat.thread.readingCli", { name: planner?.label ?? t("chat.thread.thePlanner") })}{/if}</p>
        </div>
      {:else if answeringHere && loadState === "ready"}
        <div class="planner">
          <span class="who">{t("chat.thread.planner")}</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{t("chat.thread.stillAnswering")}</p>
        </div>
      {/if}
    </div>

    <form class="composer" onsubmit={send}>
      <label class="sr-only" for="composer-input">{t("chat.composer.label")}</label>
      <FolderMention bind:this={mention} textarea={input} />
      <textarea
        id="composer-input"
        rows="2"
        bind:this={input}
        bind:value={draft}
        {onkeydown}
        disabled={missing}
        placeholder={t("chat.composer.placeholder")}
      ></textarea>
      {#if sendError}<p class="err-text" role="alert" style="margin:0">{tb(sendError)}</p>{/if}
      <div class="composer-bar">
        {#if planner && app.settings?.plannerSource !== "api"}<PlannerModel cli={planner} />{/if}
        <button
          class="btn primary send"
          type="submit"
          id="btn-send"
          aria-label={t("chat.composer.send")}
          title={t("chat.composer.sendHint")}
          disabled={pending !== null || answeringHere || !draft.trim() || !canPlan || missing}
        >
          <PaperPlaneTilt size={16} weight="fill" aria-hidden="true" />
        </button>
      </div>
      {#if pending && !thinkingHere}
        <p class="meta composer-hint" role="status">{t("chat.composer.busyElsewhere")}</p>
      {/if}
      {#if autoRun.length}
        <p class="meta composer-hint" id="chat-auto-run">
          {t("chat.composer.autoRun", { folders: autoRun.map(folderName).join(", ") })} <a class="link" href="/settings#auto-run">{t("chat.composer.autoRunChange")}</a>
        </p>
      {/if}
      {#if !canPlan}
        <p class="meta composer-hint" role="status">
          {#if provider}{t("chat.composer.providerMissing")}
          {:else if app.clisState === "loading"}{t("chat.composer.checkingClis")}
          {:else}{t("chat.composer.noPlanner")}{/if}
        </p>
      {/if}
    </form>
  </main>

  {#if railOpen && wide.current}
    <aside class="rail" id="chat-live-rail" aria-labelledby="rail-title">
      {@render live()}
    </aside>
  {/if}
</div>

{#snippet live()}
  <h2 id="rail-title">{t("chat.live.title")}</h2>
  {#if liveSessions.length === 0}
    <p class="meta" style="margin:0">{t("chat.live.empty")}</p>
  {/if}
  {#each liveSessions as s (s.id)}
    <a class="live" href="/session?id={s.id}">
      <span class="row" style="gap:8px">
        <CliMark kind={s.cli} small />
        <b class="grow" title={s.title}>{s.title}</b>
        <StatusChip status={s.status} />
      </span>
      <Horizon marks={s.marks} start={s.startedAt} end={app.now} live={s.status === "running"} full />
      <span class="meta" style="font-size:12px"><span title={s.cwd}>{t("chat.live.where", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })}</span> · {s.lastEvent ? tb(s.lastEvent) : t("chat.live.starting")}</span>
    </a>
  {/each}
{/snippet}

<style>
  .chat-wrap {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
  }
  .chat-wrap.docked {
    grid-template-columns: 240px minmax(0, 1fr) 300px;
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
  /* The tall columns beside the sidebar take the thin glass, so they read as part of the page rather than more
     sidebars. Thin glass only passes with ink, so secondary text inside them uses ink as well. */
  .history,
  .rail:not(.pop) {
    --surface: var(--glass-thin);
    --ink-2: var(--ink);
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
  /* No plate behind the header, at the owner's request. Each control carries its own glass instead, since ink over the
     bare Day sky drops to about 1.9:1. */
  .page-head {
    position: relative;
    width: fit-content;
    align-self: flex-end;
    padding: 0;
    background: none;
    box-shadow: none;
    -webkit-backdrop-filter: none;
    backdrop-filter: none;
  }
  .page-head > .btn,
  .page-head > .icon-btn {
    background: var(--surface);
    box-shadow: var(--glass-rim);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
  }
  .page-head > .btn:hover,
  .page-head > .icon-btn:hover {
    background: var(--surface-2);
  }
  /* Below 1280 the rail has no column, so it floats under its button above the conversation. */
  .rail.pop {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 5;
    width: min(300px, calc(100vw - 32px));
    max-height: min(480px, 60vh);
    padding: 20px;
    box-shadow: var(--glass-rim), var(--shadow-modal);
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
    .page-head .icon-btn {
      width: 44px;
      height: 44px;
    }
    .chat-col {
      padding: 20px 16px;
    }
    .thread {
      overflow: visible;
    }
  }
</style>
