<script module lang="ts">
  import type { ModelList } from "$lib/api";

  // Model lists per CLI and version while the phone app is open: Codex and OpenCode take seconds
  // to print theirs, so a list is asked for once; a failed one is asked for again next time.
  const modelLists = new Map<string, Promise<ModelList>>();
</script>

<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Plus from "phosphor-svelte/lib/Plus";
  import SidebarSimple from "phosphor-svelte/lib/SidebarSimple";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Trash from "phosphor-svelte/lib/Trash";
  import { onMount, tick, untrack } from "svelte";
  import type { ChatMessage, ChatModel, ChatThread, CliInstall, CliKind, EventRow, ProjectFolder } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import FolderMention from "$lib/FolderMention.svelte";
  import Horizon from "$lib/Horizon.svelte";
  import PhoneDispatchCard from "$lib/PhoneDispatchCard.svelte";
  import PlannerModel, { type ModelSource } from "$lib/PlannerModel.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, runsCli } from "$lib/format";
  import { plural, t, tb } from "$lib/i18n.svelte";
  import { hardwareKeyboard, keyboard, watchKeyboard } from "$lib/keyboard.svelte";
  import { call, notify, onMessage, phone, PhoneError } from "$lib/phone.svelte";
  import { MARK_KINDS } from "$lib/store.svelte";

  /** What `GET /api/chat/setup` says about the planner; the custom provider's address and key stay on the desktop. */
  interface Setup {
    clis: CliInstall[];
    chatCli: CliKind | null;
    plannerSource: "cli" | "api";
    provider: { model: string; ready: boolean } | null;
    chatModels: Record<string, ChatModel>;
    autoRun: string[];
  }

  // No `id` is a new chat: the first message creates its thread.
  const id = $derived(page.url.searchParams.get("id"));

  let thread = $state<ChatThread | null>(null);
  let messages = $state<ChatMessage[]>([]);
  let loadState = $state<"loading" | "ready" | "missing" | "error">("loading");
  let loadError = $state("");
  let shownId = $state<string | null | undefined>(undefined);
  let draft = $state("");
  // One planner turn at a time. It stays with its chat when the owner opens another one.
  let pending = $state<{ threadId: string | null; text: string } | null>(null);
  // Chats the planner is answering in, as the desktop knows them: a message sent before this page
  // opened, from the desktop, or from another phone.
  let answering = $state<string[]>([]);
  let threads = $state<ChatThread[]>([]);
  let threadsState = $state<"loading" | "ready" | "error">("loading");
  let threadsError = $state("");
  let setup = $state<Setup | null>(null);
  let setupState = $state<"loading" | "ready" | "error">("loading");
  let setupError = $state("");
  // Folders whose cards start without Run; the note under the composer says so (PRD FR-26).
  let autoRun = $state<string[]>([]);
  // What a screen reader hears when an answer lands; the thread itself is not a live region.
  let announcement = $state("");
  let sendError = $state("");
  let confirmDelete = $state(false);
  let deleting = $state(false);
  let historyOpen = $state(false);
  let liveOpen = $state(false);
  // Horizon marks of the live sessions, `[at, kind]` newest first.
  let marks = $state<Record<string, [number, string][]>>({});
  let now = $state(Date.now());
  let dockHeight = $state(0);
  let input: HTMLTextAreaElement | undefined = $state();
  let mention: FolderMention | undefined = $state();

  const thinkingHere = $derived(pending !== null && pending.threadId === id);
  const answeringHere = $derived(!thinkingHere && id !== null && answering.includes(id));
  const liveSessions = $derived(phone.sessions.filter(runsCli));

  // The same choice as the desktop: the chat CLI from Settings when it can plan, else the first one that can.
  const planner = $derived.by(() => {
    if (!setup) return null;
    const usable = setup.clis.filter((c) => c.path && c.kind !== "gemini");
    return usable.find((c) => c.kind === setup?.chatCli) ?? usable[0] ?? null;
  });
  const provider = $derived(setup?.plannerSource === "api" ? (setup.provider ?? { model: "", ready: false }) : null);
  // A setup that could not be read lets Send through: the desktop then answers with its own reason.
  const canPlan = $derived(setupState === "error" || (provider ? provider.ready : planner !== null));

  const models: ModelSource = {
    saved: (kind) => setup?.chatModels[kind] ?? { model: "", effort: "" },
    list(cli) {
      const key = `${cli.kind}@${cli.version ?? ""}`;
      let list = modelLists.get(key);
      if (!list) {
        list = call<ModelList>(`/api/chat/models/${cli.kind}`);
        modelLists.set(key, list);
        list.catch(() => modelLists.delete(key));
      }
      return list;
    },
    forget: () => modelLists.clear(),
    async save(kind, model, effort) {
      const r = await call<{ chatModels: Record<string, ChatModel> }>("/api/chat/model", {
        method: "POST",
        body: JSON.stringify({ cli: kind, model, effort }),
      });
      return () => {
        if (setup) setup.chatModels = r.chatModels;
      };
    },
    failed: notify,
  };

  const folders = async () => (await call<{ folders: ProjectFolder[] }>("/api/folders")).folders;
  const failText = (e: unknown) => (e instanceof Error ? e.message : String(e));

  async function scrollDown() {
    await tick();
    window.scrollTo({ top: document.body.scrollHeight });
  }

  async function loadThreads(quiet = false) {
    if (!quiet) threadsState = "loading";
    try {
      const r = await call<{ threads: ChatThread[]; answering?: string[] }>("/api/chat/threads");
      threads = r.threads;
      answering = r.answering ?? [];
      threadsState = "ready";
    } catch (e) {
      if (quiet) return;
      threadsState = "error";
      threadsError = failText(e);
    }
  }

  async function loadSetup(quiet = false) {
    if (!quiet) setupState = "loading";
    try {
      const r = await call<Setup>("/api/chat/setup");
      setup = r;
      autoRun = r.autoRun ?? [];
      setupState = "ready";
    } catch (e) {
      if (quiet) return;
      setupState = "error";
      setupError = failText(e);
    }
  }

  async function loadMarks() {
    try {
      const r = await call<{ marks?: Record<string, [number, string][]> }>("/api/sessions");
      marks = r.marks ?? {};
    } catch {
      // The list still shows each session; its horizon fills from live events.
    }
  }

  function addMark(row: EventRow) {
    if (!MARK_KINDS.has(row.event.kind)) return;
    marks[row.sessionId] = [[row.at, row.event.kind] as [number, string], ...(marks[row.sessionId] ?? [])].slice(0, 40);
    // The horizon ends at `now`; a mark after it would wait for the next tick to show.
    now = Math.max(now, row.at);
  }

  /** `quiet` reloads the chat on screen without the loading state, after a change elsewhere. */
  async function open(target: string | null, quiet = false) {
    shownId = target;
    if (!quiet) {
      confirmDelete = false;
      sendError = "";
      // The title from the list shows while the messages load.
      thread = threads.find((x) => x.id === target) ?? null;
    }
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
      // The desktop stores the question before the planner answers; the pending bubble shows it already.
      const last = r.messages.at(-1);
      const asked = pending?.threadId === target && last?.role === "user" && last.text === pending.text;
      const before = messages.length;
      thread = r.thread;
      messages = asked ? r.messages.slice(0, -1) : r.messages;
      answering = r.answering ? [...answering.filter((x) => x !== target), target] : answering.filter((x) => x !== target);
      if (r.autoRun) autoRun = r.autoRun;
      loadState = "ready";
      if (quiet && messages.length > before && messages.at(-1)?.role !== "user") announce(messages.at(-1)!);
      if (!quiet) scrollDown();
    } catch (e) {
      if (shownId !== target) return;
      if (e instanceof PhoneError && e.status === 404) {
        thread = null;
        loadState = "missing";
      } else if (!quiet) {
        loadState = "error";
        loadError = failText(e);
      }
    }
  }

  $effect(() => {
    const target = id;
    untrack(() => {
      if (target !== shownId) open(target);
    });
  });

  onMount(() => {
    loadThreads();
    loadSetup();
    const stopKeys = watchKeyboard();
    const timer = setInterval(() => (now = Date.now()), 30_000);
    // A turn started, finished or a card changed: on the desktop, in another chat, or on another
    // phone. The turn this page is waiting for merges its own answer, so only other changes reload.
    const off = onMessage((m) => {
      if (m.type === "chat") {
        loadThreads(true);
        if (m.threadId && m.threadId === shownId && pending?.threadId !== shownId && !deleting) open(m.threadId, true);
      } else if (m.type === "settings") {
        loadSetup(true);
      } else if (m.type === "event" && m.event) {
        addMark(m.event);
      } else if (m.type === "resync") {
        loadThreads(true);
        if (liveOpen) loadMarks();
      }
    });
    return () => {
      off();
      stopKeys();
      clearInterval(timer);
    };
  });

  function keep(chat: ChatThread) {
    threads = [chat, ...threads.filter((x) => x.id !== chat.id)];
  }

  async function send(e?: SubmitEvent) {
    e?.preventDefault();
    const text = draft.trim();
    if (!text || pending || answeringHere || !canPlan) return;
    const threadId = id;
    pending = { threadId, text };
    draft = "";
    sendError = "";
    scrollDown();
    try {
      const turn = await call<{ thread: ChatThread; user: ChatMessage; reply: ChatMessage }>("/api/chat", {
        method: "POST",
        body: JSON.stringify({ threadId, message: text }),
      });
      keep(turn.thread);
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
      if (shownId === threadId) {
        sendError = failText(err);
        draft = text;
      } else {
        notify(t("chat.thread.otherChatFailed", { error: tb(failText(err)) }));
      }
    } finally {
      pending = null;
      scrollDown();
      // Back to the composer only with a hardware keyboard: on a touch screen it would pop the
      // on-screen keyboard up again when the answer lands.
      const at = document.activeElement;
      if (hardwareKeyboard() && (!at || at === document.body || at === input)) input?.focus();
    }
  }

  function announce(m: ChatMessage) {
    const n = m.cards.length;
    announcement = m.role === "error" ? tb(m.text) : n ? plural(n, "phone.thread.answeredOne", "phone.thread.answeredMany") : t("phone.thread.answered");
  }

  function replace(m: ChatMessage) {
    messages = messages.map((x) => (x.id === m.id ? m : x));
  }

  async function deleteChat() {
    const gone = thread;
    if (!gone) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    deleting = true;
    try {
      await call(`/api/chat/${gone.id}`, { method: "DELETE" });
      threads = threads.filter((x) => x.id !== gone.id);
      notify(t("chat.delete.done", { title: gone.title }));
      await goto("/m/chat", { replaceState: true });
    } catch (err) {
      confirmDelete = false;
      notify(t("chat.delete.failed", { error: tb(failText(err)) }));
    } finally {
      deleting = false;
    }
  }

  function openLive() {
    liveOpen = true;
    loadMarks();
  }

  function onkeydown(e: KeyboardEvent) {
    if (mention?.keydown(e)) return;
    // Enter sends from a hardware keyboard only; on a touch keyboard it adds a line and Send sends.
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing && hardwareKeyboard()) {
      e.preventDefault();
      send();
    }
  }
</script>

<svelte:head><title>{thread ? `${thread.title} · ` : ""}{t("phone.nav.chat")} · OpenCompanion</title></svelte:head>

<header class="bar">
  <a class="back" href="/m/chat"><CaretLeft size={20} aria-hidden="true" />{t("phone.thread.back")}</a>
  <span class="grow"></span>
  <button class="icon-btn m-bar-btn" type="button" id="btn-chat-history" aria-haspopup="dialog" aria-label={t("phone.thread.history")} onclick={() => (historyOpen = true)}>
    <ChatsCircle size={22} aria-hidden="true" />
  </button>
  <button
    class="icon-btn m-bar-btn"
    type="button"
    id="btn-live-rail"
    aria-haspopup="dialog"
    aria-label={liveSessions.length ? t("phone.thread.liveCount", { n: liveSessions.length }) : t("chat.live.title")}
    onclick={openLive}
  >
    <SidebarSimple size={22} mirrored aria-hidden="true" />
    {#if liveSessions.length}<span class="m-count" aria-hidden="true">{liveSessions.length}</span>{/if}
  </button>
</header>
<main class="content" id="phone-chat-thread" style:padding-bottom="{dockHeight + keyboard.inset + 24}px">
  <div class="head-row m-thread-head">
    <h1 class="m-title">{thread?.title ?? t("phone.newChat")}</h1>
    {#if thread && loadState === "ready"}
      <button class="btn ghost" type="button" id="btn-delete-chat" onclick={deleteChat} disabled={thinkingHere || answeringHere || deleting}>
        <Trash size={16} aria-hidden="true" />{confirmDelete ? t("chat.delete.confirm") : t("chat.delete.button")}
      </button>
    {/if}
  </div>

  {#if loadState === "missing"}
    <p class="m-p" role="alert">{t("phone.thread.deleted")}</p>
    <a class="btn secondary block" href="/m/chat/thread">{t("phone.thread.startNew")}</a>
  {:else if loadState === "loading"}
    <p class="m-p" role="status">{t("phone.thread.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{t("phone.thread.loadFailed", { error: tb(loadError) })}</p>
    <button class="btn secondary block" type="button" onclick={() => open(id)}>{t("phone.tryAgain")}</button>
  {:else}
    <p class="sr-only" role="status" id="chat-announcement">{announcement}</p>
    <div class="m-thread">
      {#if messages.length === 0 && !thinkingHere}
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
              <PhoneDispatchCard card={c} messageId={m.id} clis={setup?.clis ?? []} {now} onchange={replace} />
            {/each}
          </div>
        {/if}
      {/each}
      {#if thinkingHere && pending}
        <p class="m-me">{pending.text}</p>
        <div class="m-planner">
          <span class="who">{t("phone.thread.planner")}</span>
          <p class="thinking" role="status">
            <SpinnerGap size={18} class="spin" aria-hidden="true" />
            {#if provider}{t("chat.thread.readingProvider", { name: provider.model || t("chat.thread.customProvider") })}
            {:else}{t("chat.thread.readingCli", { name: planner?.label ?? t("chat.thread.thePlanner") })}{/if}
          </p>
        </div>
      {:else if answeringHere}
        <div class="m-planner">
          <span class="who">{t("phone.thread.planner")}</span>
          <p class="thinking" role="status"><SpinnerGap size={18} class="spin" aria-hidden="true" />{t("phone.thread.stillAnswering")}</p>
        </div>
      {/if}
    </div>
  {/if}
</main>

{#if loadState === "ready"}
  <div class="dock" class:keys-up={keyboard.inset > 0} id="phone-chat-composer" bind:offsetHeight={dockHeight} style:bottom="{keyboard.inset}px">
    <form class="stack m-composer" onsubmit={send}>
      <label class="sr-only" for="pc-input">{t("phone.thread.inputLabel")}</label>
      <FolderMention bind:this={mention} textarea={input} list={folders} canBrowse={false} />
      <textarea
        class="textarea"
        id="pc-input"
        rows="2"
        bind:this={input}
        bind:value={draft}
        {onkeydown}
        placeholder={t("chat.composer.placeholder")}
      ></textarea>
      {#if sendError}<p class="err-text small" role="alert">{tb(sendError)}</p>{/if}
      <div class="m-composer-bar">
        {#if planner && setup?.plannerSource !== "api"}<PlannerModel cli={planner} source={models} />{/if}
        <button class="btn primary" type="submit" id="btn-chat-send" disabled={pending !== null || answeringHere || !draft.trim() || !canPlan}>
          <PaperPlaneTilt size={16} weight="fill" aria-hidden="true" />{t("phone.send")}
        </button>
      </div>
      {#if pending && !thinkingHere}<p class="small" role="status">{t("chat.composer.busyElsewhere")}</p>{/if}
      {#if setupState === "error"}
        <p class="small" role="alert">{t("phone.thread.setupFailed", { error: tb(setupError) })}</p>
        <button class="btn ghost" type="button" onclick={() => loadSetup()}>{t("phone.tryAgain")}</button>
      {:else if !canPlan}
        <p class="small" role="status">
          {#if provider}{t("chat.composer.providerMissing")}
          {:else if setupState === "loading"}{t("chat.composer.checkingClis")}
          {:else}{t("chat.composer.noPlanner")}{/if}
        </p>
      {/if}
      {#if autoRun.length}<p class="small" id="chat-auto-run">{t("phone.thread.autoRun", { folders: autoRun.map(folderName).join(", ") })}</p>
      {:else}<p class="small">{t("phone.thread.noAutoRun")}</p>{/if}
    </form>
  </div>
{/if}

<Dialog bind:open={historyOpen} labelledby="chat-history-title" sheet>
  <div class="d-body m-sheet" id="chat-history">
    <div class="head-row">
      <h2 id="chat-history-title" class="grow">{t("phone.thread.history")}</h2>
      <a class="btn secondary" href="/m/chat/thread" id="btn-sheet-new-chat" onclick={() => (historyOpen = false)}><Plus size={16} aria-hidden="true" />{t("phone.newChat")}</a>
    </div>
    {#if threadsState === "loading"}
      <p class="m-p" role="status">{t("phone.chat.loading")}</p>
    {:else if threadsState === "error"}
      <p class="err-text" role="alert" style="margin:0">{t("phone.chat.loadFailed", { error: tb(threadsError) })}</p>
      <button class="btn secondary" type="button" onclick={() => loadThreads()}>{t("phone.tryAgain")}</button>
    {:else if threads.length === 0}
      <p class="m-p">{t("phone.chat.empty")}</p>
    {:else}
      <nav class="m-list" aria-label={t("phone.chat.saved")}>
        {#each threads as chat (chat.id)}
          <a href="/m/chat/thread?id={chat.id}" aria-current={chat.id === id ? "page" : undefined} onclick={() => (historyOpen = false)}>
            <b>{chat.title}</b>
            <span class="meta">{pending?.threadId === chat.id || answering.includes(chat.id) ? t("phone.chat.answering") : ago(chat.updatedAt, now)}</span>
          </a>
        {/each}
      </nav>
    {/if}
    <button class="btn ghost" type="button" onclick={() => (historyOpen = false)}>{t("phone.thread.close")}</button>
  </div>
</Dialog>

<Dialog bind:open={liveOpen} labelledby="chat-live-title" sheet>
  <div class="d-body m-sheet" id="chat-live-rail">
    <h2 id="chat-live-title">{t("chat.live.title")}</h2>
    {#if liveSessions.length === 0}
      <p class="m-p">{t("chat.live.empty")}</p>
    {:else}
      <div class="m-live-list">
        {#each liveSessions as s (s.id)}
          <a class="m-live" href="/m/session?id={s.id}" onclick={() => (liveOpen = false)}>
            <span class="row" style="gap:8px">
              <CliMark kind={s.cli} small />
              <b class="grow">{s.title}</b>
              <StatusChip status={s.status} />
            </span>
            <Horizon marks={marks[s.id] ?? []} start={s.startedAt} end={now} live={s.status === "running"} full />
            <span class="meta">{t("chat.live.where", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd) })} · {s.lastEvent ? tb(s.lastEvent) : t("chat.live.starting")}</span>
          </a>
        {/each}
      </div>
    {/if}
    <button class="btn ghost" type="button" onclick={() => (liveOpen = false)}>{t("phone.thread.close")}</button>
  </div>
</Dialog>
