<script lang="ts">
  import { goto } from "$app/navigation";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import { errorText, type CliKind } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import { shortPath } from "$lib/format";
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

<svelte:head><title>Welcome · OpenCompanion</title></svelte:head>

<main class="scene" id="onboarding">
  <img class="scene-art" src={meadow} alt="" aria-hidden="true" />
  <section class="panel" aria-labelledby="ob-title">
    <div class="brand"><span>OpenCompanion</span><Cloud size={20} aria-hidden="true" /></div>
    <div>
      <h1 id="ob-title">{app.clisState === "ready" && planners.length === 0 ? "No coding CLI was found yet" : "These coding CLIs are on your computer"}</h1>
      <p class="lead">OpenCompanion looked through your PATH and common install folders. Nothing was installed or changed.</p>
    </div>

    {#if app.clisState === "error"}
      <p class="err-text" role="alert" style="margin:0">{app.clisError}</p>
    {/if}
    <ul class="found" aria-busy={app.checkingClis || app.clisState === "loading"} aria-label="Coding CLIs found">
      {#if app.clisState === "loading"}
        <li><span class="wait-txt">Checking claude, codex, opencode and gemini…</span></li>
      {/if}
      {#each app.clis as c (c.kind)}
        <li>
          <CliMark kind={c.kind} />
          <span class="grow">
            <b>{c.label}</b>
            {#if c.path}<span class="path" title={c.path}>{shortPath(c.path)}</span>{:else}<span class="wait-txt">You can add it later from CLIs</span>{/if}
          </span>
          {#if c.path}
            <span class="ver">{c.version ?? ""}</span>
            <span class="yes"><CheckCircle size={18} aria-label="Found" /></span>
          {:else}
            <span class="chip idle">Not found</span>
          {/if}
        </li>
      {/each}
    </ul>

    {#if planners.length}
      <div class="field">
        <label class="label" for="ob-planner">Planner for Chat</label>
        <select class="select" id="ob-planner" value={pick} onchange={(e) => (chosen = e.currentTarget.value as CliKind)}>
          {#each planners as c (c.kind)}<option value={c.kind}>{c.label} {c.version ?? ""}</option>{/each}
        </select>
        <p class="help">Chat runs this CLI headless, with read-only access to your project folders, to turn your requests into session suggestions. You can change it later in Settings.</p>
      </div>
    {:else if app.clisState === "ready"}
      <p class="help" style="margin:0">Install Claude Code, Codex CLI or OpenCode in your own terminal, then press Rescan. The CLIs screen lists the install commands.</p>
    {/if}

    {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}
    <div class="actions">
      <button class="btn primary" type="button" id="ob-continue" disabled={busy || app.clisState === "loading"} onclick={finish}>
        {pickLabel ? `Continue with ${pickLabel}` : "Continue without a planner"}
      </button>
      <button class="btn secondary" type="button" disabled={app.checkingClis} onclick={refreshClis}>
        <ArrowClockwise size={16} aria-hidden="true" /><span>{app.checkingClis ? "Scanning…" : "Rescan"}</span>
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
  }
  .scene-art {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: 70% 60%;
    z-index: 0;
  }
  :global(:root[data-theme="dark"]) .scene-art {
    filter: brightness(0.42) saturate(0.85);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme="light"])) .scene-art {
      filter: brightness(0.42) saturate(0.85);
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
