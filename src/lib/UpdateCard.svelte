<script lang="ts">
  import ArrowCircleUp from "phosphor-svelte/lib/ArrowCircleUp";
  import { api } from "./api";
  import { t } from "./i18n.svelte";
  import { askInstall, progressText, update } from "./update.svelte";

  // A newer release, under the nav. Later hides it until the next version; an install started
  // from Settings shows its progress here too. Its title opens Settings › Updates.
  const v = $derived(update.view);
  const version = $derived(v?.available ?? "");
  const failed = $derived(v?.phase === "idle" && v.error?.step === "install");
  const shown = $derived(Boolean(v?.available && (!v.dismissed || v.phase !== "idle" || failed)));

  const title = $derived.by(() => {
    if (v?.phase === "downloading") return t("shell.update.downloading", { version });
    if (v?.phase === "installing") return t("shell.update.installing", { version });
    if (failed) return t("shell.update.failed");
    return t("shell.update.available", { version });
  });

  function later() {
    api.dismissUpdate().catch(() => undefined);
  }
</script>

{#if v && shown}
  <section class="upd" id="update-card" aria-labelledby="update-card-title">
    <a class="upd-head" href="/settings?s=updates" id="update-card-title" title={title}>
      <ArrowCircleUp size={18} aria-hidden="true" />
      <span class="lbl">{title}</span>
    </a>
    <div class="upd-body">
      {#if v.phase === "downloading"}
        <progress max={v.total ?? undefined} value={v.total ? v.received : undefined} aria-labelledby="update-card-title"></progress>
        <p class="upd-meta">{progressText(v)}</p>
      {:else if v.phase === "installing"}
        <p class="upd-meta" role="status">{t("shell.update.installingNote")}</p>
      {:else}
        {#if failed}
          <p class="err-text upd-err" role="alert" title={v.error?.message}>{v.error?.message}</p>
        {:else}
          <p class="upd-meta">{t("shell.update.restarts")}</p>
        {/if}
        <div class="upd-actions">
          <button class="btn secondary sm" type="button" id="btn-update-install" onclick={askInstall}>
            {failed ? t("shell.update.retry") : t("shell.update.install")}
          </button>
          <button class="btn ghost sm" type="button" id="btn-update-later" onclick={later}>{t("shell.update.later")}</button>
        </div>
      {/if}
    </div>
  </section>
{/if}

<style>
  /* A notice in the nav column, not the screen's focal point: a thin frame, and forest only on the
     icon that marks a real state. */
  .upd {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .upd-head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    color: var(--ink);
    font-size: 13px;
    font-weight: 700;
    line-height: 1.35;
  }
  .upd-head :global(svg) {
    flex: none;
    color: var(--forest);
  }
  .upd-head:hover .lbl {
    text-decoration: underline;
  }
  .upd-meta {
    margin: 0;
    font-size: 12px;
    color: var(--ink-2);
  }
  /* The updater's own message can be long; the full text stays in the tooltip and in Settings. */
  .upd-err {
    margin: 0;
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .upd-actions {
    display: flex;
    gap: 6px;
  }
  progress {
    width: 100%;
    height: 6px;
    accent-color: var(--forest);
  }
  /* Icons-only sidebar: the card shrinks to its icon, a link to Settings › Updates. */
  @media (max-width: 1100px) {
    .upd {
      padding: 8px;
      align-items: center;
    }
    .upd-body {
      display: none;
    }
  }
</style>
