<script lang="ts">
  import Kanban from "phosphor-svelte/lib/Kanban";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import Play from "phosphor-svelte/lib/Play";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import type { ChatMessage, DispatchCard } from "./api";
  import CliMark from "./CliMark.svelte";
  import StatusChip from "./StatusChip.svelte";
  import { CLI_LABEL, folderName, shortPath } from "./format";
  import { call, notify, phone } from "./phone.svelte";

  // The phone version of a Chat card (PRD FR-57): the same Run, Add to board and Discard as the desktop.
  // Editing a card stays on the desktop.
  let { card, messageId, onchange }: { card: DispatchCard; messageId: string; onchange: (m: ChatMessage) => void } = $props();

  let busy = $state(false);
  let failure = $state("");

  const session = $derived(card.sessionId ? phone.sessions.find((s) => s.id === card.sessionId) : undefined);
  const labelId = $derived(`pcard-${card.id}`);
  const follow = $derived(Boolean(card.target));

  async function post<T>(path: string, extra: Record<string, unknown> = {}) {
    busy = true;
    failure = "";
    try {
      return await call<T>(path, { method: "POST", body: JSON.stringify({ messageId, cardId: card.id, ...extra }) });
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
      return null;
    } finally {
      busy = false;
    }
  }

  async function update(path: string, extra: Record<string, unknown> = {}, done?: string) {
    const r = await post<{ message: ChatMessage }>(path, extra);
    if (!r) return;
    onchange(r.message);
    if (done) notify(done);
  }

  const run = () =>
    update("/api/chat/cards/run", {}, follow ? `Sent to ${CLI_LABEL[card.cli]} in ${folderName(card.folder)}.` : `Started ${CLI_LABEL[card.cli]} in ${folderName(card.folder)}.`);
  const discard = () => update("/api/chat/cards/discard");
  const undo = () => update("/api/chat/cards/discard", { undo: true });

  const toBoard = () => update("/api/chat/cards/board", {}, "Added to the Board in Todo.");
</script>

{#if card.state === "discarded"}
  <div class="m-discarded">
    <span class="grow">Discarded the {CLI_LABEL[card.cli]} card for {folderName(card.folder)}.</span>
    <button class="btn secondary" type="button" disabled={busy} onclick={undo}>Undo</button>
  </div>
  {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}
{:else}
  <article class="m-dcard" aria-labelledby={labelId}>
    <div class="row" style="gap:10px;align-items:flex-start">
      <CliMark kind={card.cli} />
      <div class="target grow">
        <b id={labelId}>{follow ? `Follow-up for ${card.title}` : card.title || `${CLI_LABEL[card.cli]} · ${card.mode}`}</b>
        <span>{#if card.title}{CLI_LABEL[card.cli]} · {card.mode} · {/if}<span class="mono">{shortPath(card.folder)}</span></span>
      </div>
      {#if session}<StatusChip status={session.status} />{:else if card.state === "started"}<span class="chip idle">{follow ? "Sent" : "Started"}</span>{:else}<span class="chip idle">{follow ? "Follow-up" : "Ready"}</span>{/if}
    </div>
    <p class="prompt">{card.prompt}</p>
    {#if card.reason}<p class="why">{card.reason}</p>{/if}
    {#if card.problem && card.state === "proposed"}<p class="err-text" style="margin:0">{card.problem}</p>{/if}
    {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

    <div class="acts">
      {#if card.state === "started"}
        {#if card.sessionId}
          <a class="btn secondary wide" href="/m/session?id={card.sessionId}"><TerminalWindow size={16} aria-hidden="true" />Open session</a>
        {/if}
      {:else}
        <button class="btn primary wide" type="button" disabled={busy || Boolean(card.problem)} onclick={run}>
          {#if follow}<PaperPlaneTilt size={16} weight="fill" aria-hidden="true" />Send to session{:else}<Play size={16} aria-hidden="true" />Run in {folderName(card.folder) || "…"}{/if}
        </button>
        {#if follow}
          <!-- A follow-up belongs to its session, so it has no Board card. -->
        {:else if card.taskId}
          <a class="btn secondary" href="/m/board"><Kanban size={16} aria-hidden="true" />On the Board</a>
        {:else}
          <button class="btn secondary" type="button" disabled={busy} onclick={toBoard}><Kanban size={16} aria-hidden="true" />Add to board</button>
        {/if}
        <button class="btn ghost" type="button" disabled={busy} onclick={discard}>Discard</button>
      {/if}
    </div>
  </article>
{/if}
