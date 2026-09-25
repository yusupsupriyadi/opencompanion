<script lang="ts">
  import Check from "phosphor-svelte/lib/Check";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import X from "phosphor-svelte/lib/X";
  import { api, errorText, type SessionInfo } from "./api";
  import CliMark from "./CliMark.svelte";
  import { CLI_LABEL, ago, folderName, shortPath, waitingTitle } from "./format";
  import { app, showToast } from "./store.svelte";

  // The one focal point of the Overview (DESIGN.md D2), also used compact on Session detail.
  let { s, compact = false, showOpen = true }: { s: SessionInfo; compact?: boolean; showOpen?: boolean } = $props();

  let busy = $state(false);
  let failure = $state("");
  const w = $derived(s.waiting);
  const titleId = $derived(`needs-${s.id}`);

  async function answer(allow: boolean) {
    busy = true;
    failure = "";
    try {
      await api.answerSession(s.id, allow);
      showToast(allow ? `Approved. ${CLI_LABEL[s.cli]} continues in ${folderName(s.cwd)}.` : `Denied. ${CLI_LABEL[s.cli]} was told no.`);
    } catch (e) {
      failure = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="needs" class:compact aria-labelledby={titleId}>
  <div class="row">
    {#if !compact}<CliMark kind={s.cli} />{/if}
    <div class="grow">
      <h2 id={titleId}>{waitingTitle(s)}</h2>
      <div class="meta">
        {folderName(s.cwd)} · <span class="mono">{shortPath(s.cwd)}</span>
        {#if w} · asked {ago(w.since, app.now)}{/if}
      </div>
    </div>
    <span class="chip wait">Waiting for you</span>
  </div>
  {#if w?.detail}
    <div class="cmd"><small>{w.tool ?? (w.reason === "permission" ? "Request" : "On screen")}</small><code>{w.detail}</code></div>
  {/if}
  {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}
  <div class="actions">
    {#if w?.canAnswer}
      <button class="btn accent" type="button" disabled={busy} onclick={() => answer(true)}><Check size={16} aria-hidden="true" />Approve</button>
      <button class="btn secondary" type="button" disabled={busy} onclick={() => answer(false)}><X size={16} aria-hidden="true" />Deny</button>
    {:else}
      <span class="meta">Answer this in the session's terminal. OpenCompanion does not pick an option for you here.</span>
    {/if}
    <span class="spacer"></span>
    {#if showOpen}
      <a class="btn secondary" href="/session?id={s.id}"><TerminalWindow size={16} aria-hidden="true" />Open session</a>
    {/if}
  </div>
</section>

<style>
  .needs.compact {
    padding: 16px;
    gap: 12px;
  }
  .needs.compact h2 {
    font-size: 16px;
  }
</style>
