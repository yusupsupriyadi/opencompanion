<script lang="ts">
  import Kanban from "phosphor-svelte/lib/Kanban";
  import PaperPlaneTilt from "phosphor-svelte/lib/PaperPlaneTilt";
  import PencilSimple from "phosphor-svelte/lib/PencilSimple";
  import Play from "phosphor-svelte/lib/Play";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import { tick } from "svelte";
  import type { ChatMessage, CliInstall, CliKind, DispatchCard, Mode } from "./api";
  import CliMark from "./CliMark.svelte";
  import { refocus } from "./focus";
  import StatusChip from "./StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, shortPath } from "./format";
  import { t, tb } from "./i18n.svelte";
  import { call, notify, phone } from "./phone.svelte";

  // The phone version of a Chat card (PRD FR-57): the same Run, Edit, Add to board and Discard as
  // the desktop. `clis` fills the CLI picker while editing; `now` ticks "Started 3 min ago".
  let {
    card,
    messageId,
    clis = [],
    now = Date.now(),
    onchange,
  }: { card: DispatchCard; messageId: string; clis?: CliInstall[]; now?: number; onchange: (m: ChatMessage) => void } = $props();

  let editing = $state(false);
  let busy = $state(false);
  let failure = $state("");
  let draft = $state({ cli: "claude" as CliKind, title: "", folder: "", prompt: "", mode: "headless" as Mode });
  let slot: HTMLDivElement | undefined = $state();

  const session = $derived(card.sessionId ? phone.sessions.find((s) => s.id === card.sessionId) : undefined);
  const labelId = $derived(`pcard-${card.id}`);
  // A follow-up goes to a session that already runs (PRD FR-25); it starts nothing.
  const follow = $derived(Boolean(card.target));
  // A card that ran or went away elsewhere while it was open for editing shows what it is now.
  const editingNow = $derived(editing && card.state === "proposed");
  // Before the desktop has listed its CLIs, the picker still offers the card's own.
  const cliOptions = $derived(
    clis.some((c) => c.kind === card.cli) ? clis : [{ kind: card.cli, label: CLI_LABEL[card.cli], path: "" } as CliInstall, ...clis],
  );

  async function post<T>(path: string, body: Record<string, unknown>) {
    busy = true;
    failure = "";
    try {
      return await call<T>(path, { method: "POST", body: JSON.stringify({ messageId, ...body }) });
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
      return null;
    } finally {
      busy = false;
    }
  }

  async function update(path: string, extra: Record<string, unknown> = {}, done?: string) {
    const r = await post<{ message: ChatMessage }>(path, { cardId: card.id, ...extra });
    if (!r) return;
    onchange(r.message);
    if (done) notify(done);
    // Run, Discard and Undo swap the buttons; focus moves to what took their place.
    refocus(slot);
  }

  const run = () => {
    const where = { cli: CLI_LABEL[card.cli], folder: folderName(card.folder) };
    return update("/api/chat/cards/run", {}, follow ? t("phone.card.sent", where) : t("phone.card.started", where));
  };
  const discard = () => update("/api/chat/cards/discard");
  const undo = () => update("/api/chat/cards/discard", { undo: true });
  const toBoard = () => update("/api/chat/cards/board", {}, t("phone.card.addedToBoard"));

  async function edit() {
    draft = { cli: card.cli, title: card.title, folder: card.folder, prompt: card.prompt, mode: card.mode };
    failure = "";
    editing = true;
    await tick();
    slot?.querySelector<HTMLElement>("input, textarea")?.focus();
  }

  async function save() {
    const r = await post<{ message: ChatMessage }>("/api/chat/cards/edit", { card: { ...card, ...draft } });
    if (!r) return;
    editing = false;
    onchange(r.message);
    refocus(slot);
  }

  function cancel() {
    editing = false;
    failure = "";
    refocus(slot);
  }
</script>

<div class="slot" bind:this={slot}>
{#if card.state === "discarded"}
  <div class="m-discarded">
    <span class="grow">{t("phone.card.discarded", { cli: CLI_LABEL[card.cli], folder: folderName(card.folder) })}</span>
    <button class="btn secondary" type="button" disabled={busy} onclick={undo}>{t("phone.card.undo")}</button>
  </div>
  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}
{:else}
  <article class="m-dcard" aria-labelledby={labelId}>
    <div class="row" style="gap:10px;align-items:flex-start">
      <CliMark kind={editingNow && !follow ? draft.cli : card.cli} />
      <div class="target grow">
        <b id={labelId}>{follow ? t("phone.card.followUpFor", { title: card.title }) : card.title || `${CLI_LABEL[card.cli]} · ${t(`phone.mode.${card.mode}`)}`}</b>
        <span>{#if card.title}{CLI_LABEL[card.cli]} · {t(`phone.mode.${card.mode}`)} · {/if}<span class="mono">{shortPath(card.folder)}</span></span>
      </div>
      {#if session}<StatusChip status={session.status} />{:else if card.state === "started"}<span class="chip idle">{follow ? t("phone.card.chipSent") : t("phone.card.chipStarted")}</span>{:else}<span class="chip idle">{follow ? t("phone.card.chipFollowUp") : t("phone.card.chipReady")}</span>{/if}
    </div>

    {#if editingNow && follow}
      <label class="field">
        <span class="label">{t("chat.card.messageLabel")}</span>
        <textarea class="textarea mono" bind:value={draft.prompt}></textarea>
      </label>
    {:else if editingNow}
      <div class="m-edit">
        <label class="field">
          <span class="label">{t("chat.card.titleLabel")}</span>
          <input class="input" bind:value={draft.title} placeholder={t("chat.card.titlePlaceholder")} autocomplete="off" />
        </label>
        <label class="field">
          <span class="label">{t("chat.card.cliLabel")}</span>
          <select class="select" bind:value={draft.cli}>
            {#each cliOptions as c (c.kind)}
              <option value={c.kind} disabled={c.kind !== card.cli && !c.path}>{c.path || c.kind === card.cli ? c.label : t("chat.card.notInstalled", { cli: c.label })}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="label">{t("chat.card.modeLabel")}</span>
          <select class="select" bind:value={draft.mode}>
            <option value="headless">{t("chat.card.optionHeadless")}</option>
            <option value="interactive">{t("chat.card.optionInteractive")}</option>
          </select>
        </label>
        <label class="field">
          <span class="label">{t("chat.card.folderLabel")}</span>
          <input class="input mono" bind:value={draft.folder} spellcheck="false" autocapitalize="off" autocomplete="off" />
        </label>
        <label class="field">
          <span class="label">{t("chat.card.promptLabel")}</span>
          <textarea class="textarea mono" bind:value={draft.prompt}></textarea>
        </label>
      </div>
    {:else}
      <p class="prompt">{card.prompt}</p>
      {#if card.reason}<p class="why">{card.reason}</p>{/if}
    {/if}

    {#if card.problem && card.state === "proposed"}<p class="err-text" style="margin:0">{tb(card.problem)}</p>{/if}
    {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

    <div class="acts">
      {#if card.state === "started"}
        {#if card.sessionId}
          <a class="btn secondary wide" href="/m/session?id={card.sessionId}"><TerminalWindow size={16} aria-hidden="true" />{t("phone.openSession")}</a>
        {/if}
        {#if follow}<p class="small wide">{t("chat.card.sentHere")}</p>
        {:else if card.auto}<p class="small wide">{t("phone.card.auto", { folder: folderName(card.folder) })}</p>
        {:else if session}<p class="small wide">{t("chat.card.startedAgo", { ago: ago(session.startedAt, now) })}</p>{/if}
      {:else if editingNow}
        <button class="btn primary" type="button" disabled={busy} onclick={save}>{t("chat.card.save")}</button>
        <button class="btn ghost" type="button" disabled={busy} onclick={cancel}>{t("chat.card.cancel")}</button>
      {:else}
        <button class="btn primary wide" type="button" disabled={busy || Boolean(card.problem)} onclick={run}>
          {#if follow}<PaperPlaneTilt size={16} weight="fill" aria-hidden="true" />{t("phone.card.sendToSession")}{:else}<Play size={16} aria-hidden="true" />{t("phone.runIn", { folder: folderName(card.folder) || "…" })}{/if}
        </button>
        <button class="btn secondary" type="button" disabled={busy} onclick={edit}><PencilSimple size={16} aria-hidden="true" />{t("chat.card.edit")}</button>
        {#if follow}
          <!-- A follow-up belongs to its session, so it has no Board card. -->
        {:else if card.taskId}
          <a class="btn secondary" href="/m/board"><Kanban size={16} aria-hidden="true" />{t("phone.card.onBoard")}</a>
        {:else}
          <button class="btn secondary" type="button" disabled={busy} onclick={toBoard}><Kanban size={16} aria-hidden="true" />{t("phone.card.addToBoard")}</button>
        {/if}
        <!-- Beside Edit on a follow-up; its own row under Edit and Add to board otherwise. -->
        <button class="btn ghost" class:wide={!follow} type="button" disabled={busy} onclick={discard}>{t("phone.card.discard")}</button>
      {/if}
    </div>
  </article>
{/if}
</div>

<style>
  .slot {
    display: contents;
  }
</style>
