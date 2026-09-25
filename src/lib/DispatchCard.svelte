<script lang="ts">
  import Kanban from "phosphor-svelte/lib/Kanban";
  import PencilSimple from "phosphor-svelte/lib/PencilSimple";
  import Play from "phosphor-svelte/lib/Play";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import { api, errorText, type ChatMessage, type CliKind, type DispatchCard, type Mode } from "./api";
  import CliMark from "./CliMark.svelte";
  import StatusChip from "./StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, shortPath } from "./format";
  import { app, showToast } from "./store.svelte";

  let { card, messageId, onchange }: { card: DispatchCard; messageId: string; onchange: (m: ChatMessage) => void } = $props();

  let editing = $state(false);
  let busy = $state(false);
  let failure = $state("");
  let draft = $state({ cli: "claude" as CliKind, title: "", folder: "", prompt: "", mode: "headless" as Mode });

  const session = $derived(card.sessionId ? app.sessions.find((s) => s.id === card.sessionId) : undefined);
  const labelId = $derived(`card-${card.id}`);

  function edit() {
    draft = { cli: card.cli, title: card.title, folder: card.folder, prompt: card.prompt, mode: card.mode };
    failure = "";
    editing = true;
  }

  async function act(what: () => Promise<ChatMessage>, done?: string) {
    busy = true;
    failure = "";
    try {
      onchange(await what());
      if (done) showToast(done);
    } catch (e) {
      failure = errorText(e);
    } finally {
      busy = false;
    }
  }

  const run = () => act(() => api.chatRunCard(messageId, card.id), `Started ${CLI_LABEL[card.cli]} in ${folderName(card.folder)}.`);
  const save = () =>
    act(async () => {
      const m = await api.chatUpdateCard(messageId, { ...card, ...draft });
      editing = false;
      return m;
    });
  const discard = () => act(() => api.chatDiscardCard(messageId, card.id));
  const undo = () => act(() => api.chatDiscardCard(messageId, card.id, true));

  async function toBoard() {
    busy = true;
    try {
      await api.chatCardToBoard(messageId, card.id);
      showToast(`Added to the Board in Todo.`);
    } catch (e) {
      failure = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if card.state === "discarded"}
  <div class="discarded">
    <span class="grow">Discarded the {CLI_LABEL[card.cli]} card for {folderName(card.folder)}.</span>
    <button class="btn secondary sm" type="button" disabled={busy} onclick={undo}>Undo</button>
  </div>
{:else}
  <article class="dcard" aria-labelledby={labelId}>
    <div class="row" style="gap:10px">
      <CliMark kind={editing ? draft.cli : card.cli} />
      <div class="target grow">
        <b id={labelId}>{card.title || `${CLI_LABEL[card.cli]} · ${card.mode}`}</b>
        <span>{#if card.title}{CLI_LABEL[card.cli]} · {card.mode} · {/if}<span class="mono" title={card.folder}>{shortPath(card.folder)}</span></span>
      </div>
      {#if session}<StatusChip status={session.status} />{:else if card.state === "started"}<span class="chip idle">Started</span>{:else}<span class="chip idle">Ready</span>{/if}
    </div>

    {#if editing}
      <div class="edit-grid">
        <label class="field span">
          <span class="label">Title</span>
          <input class="input" bind:value={draft.title} placeholder="Name the task in a few words" autocomplete="off" />
        </label>
        <label class="field">
          <span class="label">CLI</span>
          <select class="select" bind:value={draft.cli}>
            {#each app.clis as c (c.kind)}
              <option value={c.kind} disabled={!c.path}>{c.label}{c.path ? "" : " (not installed)"}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="label">Mode</span>
          <select class="select" bind:value={draft.mode}>
            <option value="headless">Headless</option>
            <option value="interactive">Interactive</option>
          </select>
        </label>
        <label class="field span">
          <span class="label">Folder</span>
          <input class="input mono" bind:value={draft.folder} spellcheck="false" />
        </label>
        <label class="field span">
          <span class="label">Prompt</span>
          <textarea class="prompt" bind:value={draft.prompt}></textarea>
        </label>
      </div>
    {:else}
      <p class="prompt">{card.prompt}</p>
      {#if card.reason}<p class="why">{card.reason}</p>{/if}
    {/if}

    {#if card.problem && card.state === "proposed"}<p class="err-text" style="margin:0">{card.problem}</p>{/if}
    {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

    <div class="acts">
      {#if card.state === "started"}
        {#if card.sessionId}
          <a class="btn secondary" href="/session?id={card.sessionId}"><TerminalWindow size={16} aria-hidden="true" />Open session</a>
        {/if}
        {#if session}<span class="meta">Started {ago(session.startedAt, app.now)}</span>{/if}
      {:else if editing}
        <button class="btn primary" type="button" disabled={busy} onclick={save}>Save card</button>
        <button class="btn ghost" type="button" onclick={() => (editing = false)}>Cancel</button>
      {:else}
        <button class="btn primary" type="button" disabled={busy || Boolean(card.problem)} onclick={run}>
          <Play size={16} aria-hidden="true" />Run in {folderName(card.folder) || "…"}
        </button>
        <button class="btn secondary" type="button" disabled={busy} onclick={edit}><PencilSimple size={16} aria-hidden="true" />Edit</button>
        <button class="btn secondary" type="button" disabled={busy} onclick={toBoard}><Kanban size={16} aria-hidden="true" />Add to board</button>
        <button class="btn ghost" type="button" disabled={busy} onclick={discard}>Discard</button>
      {/if}
    </div>
  </article>
{/if}

<style>
  .dcard {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .target b {
    display: block;
    font-size: 14px;
    font-weight: 800;
  }
  .target > span {
    font-size: 12px;
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  .prompt {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
    font: 13px/1.45 var(--font-mono);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  textarea.prompt {
    width: 100%;
    min-height: 110px;
    border: 2px solid var(--forest);
    color: var(--ink);
    resize: vertical;
  }
  .why {
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
  }
  .acts {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .edit-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .edit-grid .span {
    grid-column: 1 / -1;
  }
  .discarded {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--r-md);
    border: 1px dashed var(--line-strong);
    background: var(--surface);
    font-size: 13px;
    color: var(--ink-2);
  }
</style>
