<script lang="ts">
  import Pause from "phosphor-svelte/lib/Pause";
  import PencilSimple from "phosphor-svelte/lib/PencilSimple";
  import Play from "phosphor-svelte/lib/Play";
  import Trash from "phosphor-svelte/lib/Trash";
  import { goto } from "$app/navigation";
  import type { AutomationView } from "./api";
  import CliMark from "./CliMark.svelte";
  import { menu, openMenu } from "./context-menu.svelte";
  import { CLI_LABEL, folderName } from "./format";
  import { t } from "./i18n.svelte";
  import OutcomeChip from "./OutcomeChip.svelte";
  import { describe, whenText } from "./schedule";
  import { app } from "./store.svelte";

  let {
    a,
    busy = false,
    onrun,
    ontoggle,
    ondelete,
  }: {
    a: AutomationView;
    busy?: boolean;
    onrun: () => void;
    ontoggle: () => void;
    ondelete: () => void;
  } = $props();

  const key = $derived(`auto:${a.id}`);
  const href = $derived(`/automations?id=${a.id}`);
  const next = $derived(a.enabled && a.nextRunAt ? t("auto.nextRun", { when: whenText(a.nextRunAt, app.now) }) : t("auto.paused"));

  // Like session rows, the row carries one control and the rest lives in the right-click menu.
  function context(e: MouseEvent) {
    openMenu(e, key, t("auto.menu.label", { name: a.name }), [
      { label: t("auto.runNow"), icon: Play, action: onrun, disabled: busy },
      { label: t("auto.edit"), icon: PencilSimple, action: () => goto(href) },
      { label: t(a.enabled ? "auto.pause" : "auto.resume"), icon: a.enabled ? Pause : Play, action: ontoggle },
      null,
      { label: t("auto.delete"), icon: Trash, action: ondelete, danger: true },
    ]);
  }
</script>

<!-- The link and the switch take the keys: Shift+F10 or the Menu key on them bubbles up here. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="arow-item" class:menu-open={menu.open && menu.key === key} oncontextmenu={context}>
  <a class="srow arow" {href}>
    <CliMark kind={a.cli} />
    <span style="min-width:0">
      <span class="task" title={a.name}>{a.name}</span>
      <span class="where">{CLI_LABEL[a.cli]} · <span class="mono" title={a.cwd}>{folderName(a.cwd)}</span> · {describe(a.schedule)}</span>
    </span>
    <span class="next" class:paused={!a.enabled}>{next}</span>
    <span class="status-col">
      {#if a.lastRun}<OutcomeChip outcome={a.lastRun.outcome} />{:else}<span class="never">{t("auto.neverRan")}</span>{/if}
    </span>
  </a>
  <!-- Outside the link: a control inside <a> is invalid and would open the automation too. -->
  <button class="switch" type="button" role="switch" aria-checked={a.enabled} aria-label={t("auto.activeNamed", { name: a.name })} title={t("auto.active")} onclick={ontoggle}></button>
</div>

<style>
  .arow-item {
    position: relative;
  }
  .menu-open .arow {
    background: var(--surface-2);
  }
  .arow {
    grid-template-columns: 30px minmax(0, 1fr) 200px 110px;
    padding-right: 76px;
  }
  .next,
  .never {
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .next:not(.paused) {
    color: var(--ink);
  }
  .switch {
    position: absolute;
    top: 50%;
    right: 16px;
    translate: 0 -50%;
  }
  /* Narrow: the next run moves to a second line under the name instead of leaving the row. */
  @media (max-width: 900px) {
    .arow {
      grid-template-columns: 30px minmax(0, 1fr) 110px;
      row-gap: 4px;
    }
    .status-col {
      order: 2;
    }
    .next {
      order: 3;
      grid-column: 2 / -1;
    }
  }
</style>
