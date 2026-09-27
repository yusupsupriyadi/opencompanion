<script lang="ts">
  import { goto } from "$app/navigation";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import { errorText, type CliKind } from "$lib/api";
  import AppLogo from "$lib/AppLogo.svelte";
  import CliMark from "$lib/CliMark.svelte";
  import { shortPath } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { app, loadSettings, refreshClis, saveSettings } from "$lib/store.svelte";
  const meadow = "/meadow-day.png";

  let chosen = $state<CliKind | "">("");
  let failure = $state("");
  let busy = $state(false);

  const planners = $derived(app.clis.filter((c) => c.path && c.kind !== "gemini"));
  const pick = $derived(chosen || planners[0]?.kind || "");
  const pickLabel = $derived(app.clis.find((c) => c.kind === pick)?.label);

  async function finish() {
    busy = true;
    failure = "";
    try {
      const current = app.settings ?? (await loadSettings());
      await saveSettings({ ...current, onboarded: true, chatCli: pick || null });
      await goto("/");
    } catch (e) {
      failure = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>{t("work.onboarding.pageTitle")}</title></svelte:head>

<main class="scene" id="onboarding">
  <img class="scene-art" src={meadow} alt="" aria-hidden="true" />
  <section class="panel" aria-labelledby="ob-title">
    <div class="brand"><AppLogo /><span>OpenCompanion</span></div>
    <div>
      <h1 id="ob-title">{app.clisState === "ready" && planners.length === 0 ? t("work.onboarding.noneFound") : t("work.onboarding.found")}</h1>
      <p class="lead">{t("work.onboarding.lead")}</p>
    </div>

    {#if app.clisState === "error"}
      <p class="err-text" role="alert" style="margin:0">{tb(app.clisError)}</p>
    {/if}
    <ul class="found" aria-busy={app.checkingClis || app.clisState === "loading"} aria-label={t("work.onboarding.listLabel")}>
      {#if app.clisState === "loading"}
        <li><span class="wait-txt">{t("work.onboarding.checking")}</span></li>
      {/if}
      {#each app.clis as c (c.kind)}
        <li>
          <CliMark kind={c.kind} />
          <span class="grow">
            <b>{c.label}</b>
            {#if c.path}<span class="path" title={c.path}>{shortPath(c.path)}</span>{:else}<span class="wait-txt">{t("work.onboarding.addLater")}</span>{/if}
          </span>
          {#if c.path}
            <span class="ver">{c.version ?? ""}</span>
            <span class="yes"><CheckCircle size={18} aria-label={t("work.onboarding.foundMark")} /></span>
          {:else}
            <span class="chip idle">{t("work.onboarding.notFound")}</span>
          {/if}
        </li>
      {/each}
    </ul>

    {#if planners.length}
      <div class="field">
        <label class="label" for="ob-planner">{t("work.onboarding.plannerLabel")}</label>
        <select class="select" id="ob-planner" value={pick} onchange={(e) => (chosen = e.currentTarget.value as CliKind)}>
          {#each planners as c (c.kind)}<option value={c.kind}>{c.label} {c.version ?? ""}</option>{/each}
        </select>
        <p class="help">{t("work.onboarding.plannerHelp")}</p>
      </div>
    {:else if app.clisState === "ready"}
      <p class="help" style="margin:0">{t("work.onboarding.installHint")}</p>
    {/if}

    {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}
    <div class="actions">
      <button class="btn primary" type="button" id="ob-continue" disabled={busy || app.clisState === "loading"} onclick={finish}>
        {pickLabel ? t("work.onboarding.continueWith", { cli: pickLabel }) : t("work.onboarding.continueWithout")}
      </button>
      <button class="btn secondary" type="button" disabled={app.checkingClis} onclick={refreshClis}>
        <ArrowClockwise size={16} aria-hidden="true" /><span>{app.checkingClis ? t("work.scanning") : t("work.rescan")}</span>
      </button>
    </div>
  </section>
</main>

<style>
  .scene {
    position: relative;
    min-height: 100vh;
    display: flex;
    align-items: flex-start;
    padding: 84px 112px 48px;
    --scene-dim: brightness(1);
  }
  /* As in the app shell: the painting fits the window whole (contain), never cropped. The band left at the window's
     edges is the same painting, cover-scaled and blurred, so no flat bar shows. */
  .scene::before {
    content: "";
    position: fixed;
    inset: -48px;
    z-index: 0;
    background: var(--bg) url(/meadow-day.png) 70% 100% / cover no-repeat;
    filter: blur(32px) var(--scene-dim);
  }
  .scene-art {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    object-position: 70% 100%;
    z-index: 0;
    filter: var(--scene-dim);
  }
  :global(:root[data-theme="dark"]) .scene {
    --scene-dim: brightness(0.42) saturate(0.85);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .scene {
      --scene-dim: brightness(0.42) saturate(0.85);
    }
  }
  .panel {
    position: relative;
    z-index: 1;
    width: 540px;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 32px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .panel .brand {
    padding: 0;
    font-size: 18px;
  }
  .panel h1 {
    font-size: 30px;
  }
  .lead {
    margin: 8px 0 0;
    font-size: 15px;
    color: var(--ink-2);
  }
  .found {
    list-style: none;
    margin: 0;
    padding: 0;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--surface);
    overflow: hidden;
  }
  .found li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-top: 1px solid var(--line);
    min-height: 58px;
  }
  .found li:first-child {
    border-top: 0;
  }
  .found b {
    display: block;
    font-size: 14px;
  }
  .path {
    display: block;
    font: 12px var(--font-mono);
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  .ver {
    font: 12px var(--font-mono);
  }
  .yes {
    color: var(--forest);
    display: inline-flex;
  }
  .wait-txt {
    font-size: 13px;
    color: var(--ink-2);
  }
  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  @media (max-width: 720px) {
    .scene {
      padding: 16px;
      align-items: flex-end;
    }
    .panel {
      padding: 22px;
    }
    .panel h1 {
      font-size: 24px;
    }
  }
</style>
