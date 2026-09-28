<script lang="ts">
  import Trash from "phosphor-svelte/lib/Trash";
  import type { SessionView } from "./api";
  import CliMark from "./CliMark.svelte";
  import { menu, openMenu, sessionMenu } from "./context-menu.svelte";
  import Horizon from "./Horizon.svelte";
  import StatusChip from "./StatusChip.svelte";
  import { CLI_LABEL, isLive, shortPath, trackLine } from "./format";
  import { t } from "./i18n.svelte";
  import { app, askDelete } from "./store.svelte";

  let { s }: { s: SessionView } = $props();
  const live = $derived(s.status === "running" || s.status === "starting");
  const menuKey = $derived(`row:${s.id}`);
</script>

<!-- The link and button inside take the keys: Shift+F10 or the Menu key on them bubbles up here. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="srow-item"
  class:menu-open={menu.open && menu.key === menuKey}
  oncontextmenu={(e) => openMenu(e, menuKey, t("shell.menu.sessionLabel", { title: s.title }), sessionMenu(s))}
>
  <a class="srow" href="/session?id={s.id}">
    <CliMark kind={s.cli} />
    <span style="min-width:0">
      <span class="task" title={s.title}>{s.title}</span>
      <span class="where">{CLI_LABEL[s.cli]} · {t(`sessions.mode.${s.mode}`)} · <span class="mono" title={s.cwd}>{shortPath(s.cwd)}</span></span>
    </span>
    <span class="track">
      <Horizon marks={s.marks} start={s.startedAt} end={s.endedAt ?? app.now} {live} />
      <span class="ellipsis">{trackLine(s, app.now)}</span>
    </span>
    <span class="status-col"><StatusChip status={s.status} /></span>
  </a>
  <!-- Outside the link: a button inside <a> is invalid and would open the session too. -->
  {#if !isLive(s)}
    <button class="icon-btn del" type="button" aria-label={t("sessions.row.deleteNamed", { title: s.title })} title={t("sessions.row.delete")} onclick={() => askDelete(s)}>
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
  .menu-open .srow {
    background: var(--surface-2);
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
