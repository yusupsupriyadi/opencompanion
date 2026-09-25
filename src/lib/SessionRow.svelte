<script lang="ts">
  import Trash from "phosphor-svelte/lib/Trash";
  import type { SessionView } from "./api";
  import CliMark from "./CliMark.svelte";
  import Horizon from "./Horizon.svelte";
  import StatusChip from "./StatusChip.svelte";
  import { CLI_LABEL, isLive, shortPath, trackLine } from "./format";
  import { app, askDelete } from "./store.svelte";

  let { s }: { s: SessionView } = $props();
  const live = $derived(s.status === "running" || s.status === "starting");
</script>

<div class="srow-item">
  <a class="srow" href="/session?id={s.id}">
    <CliMark kind={s.cli} />
    <span style="min-width:0">
      <span class="task" title={s.title}>{s.title}</span>
      <span class="where">{CLI_LABEL[s.cli]} · {s.mode} · <span class="mono" title={s.cwd}>{shortPath(s.cwd)}</span></span>
    </span>
    <span class="track">
      <Horizon marks={s.marks} start={s.startedAt} end={s.endedAt ?? app.now} {live} />
      <span class="ellipsis">{trackLine(s, app.now)}</span>
    </span>
    <span class="status-col"><StatusChip status={s.status} /></span>
  </a>
  <!-- Outside the link: a button inside <a> is invalid and would open the session too. -->
  {#if !isLive(s)}
    <button class="icon-btn del" type="button" aria-label="Delete session: {s.title}" title="Delete session" onclick={() => askDelete(s)}>
      <Trash size={18} aria-hidden="true" />
    </button>
  {/if}
</div>

<style>
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .srow-item {
    position: relative;
  }
  /* Live rows keep the button's space too, so horizons stay aligned between rows. */
  .srow {
    padding-right: 66px;
  }
  .del {
    position: absolute;
    top: 50%;
    right: 16px;
    translate: 0 -50%;
  }
  .del:hover {
    color: var(--st-err);
  }
  @media (max-width: 720px) {
    .srow {
      padding-right: 68px;
    }
    .del {
      width: 44px;
      height: 44px;
      right: 12px;
    }
  }
</style>
