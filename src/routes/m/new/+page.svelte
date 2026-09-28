<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import ListBullets from "phosphor-svelte/lib/ListBullets";
  import Play from "phosphor-svelte/lib/Play";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import { onMount, tick } from "svelte";
  import type { CliKind, Mode, PermMode, SessionInfo } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import { CLI_LABEL, MODES, folderName, shortPath } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { call, type PhoneOptions } from "$lib/phone.svelte";

  // `?cwd=` starts in that folder.
  const OTHER = "__other";

  let options = $state<PhoneOptions | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");

  let cli = $state<CliKind>("claude");
  let folder = $state("");
  let typed = $state("");
  // Headless by default: its steps read well on a phone and it takes follow-ups.
  let mode = $state<Mode>("headless");
  let prompt = $state("");
  let permissionMode = $state<PermMode>("ask");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);

  const installed = $derived(options?.clis.filter((c) => c.path) ?? []);
  const cwd = $derived((folder === OTHER ? typed : folder).trim());
  const headless = $derived(mode === "headless");
  const modeInfo = $derived(MODES.find((m) => m.id === permissionMode) ?? MODES[0]);
  const label = $derived.by(() => {
    const where = cwd ? folderName(cwd) : "…";
    return t("phone.new.start", { cli: CLI_LABEL[cli], folder: where });
  });

  async function load() {
    loadState = "loading";
    try {
      const o = await call<PhoneOptions>("/api/options");
      options = o;
      const usable = o.clis.filter((c) => c.path);
      cli = usable.find((c) => c.kind !== "gemini")?.kind ?? usable[0]?.kind ?? "claude";
      const start = page.url.searchParams.get("cwd") || "";
      if (start && !o.folders.some((f) => f.path === start)) {
        folder = OTHER;
        typed = start;
      } else {
        folder = start || o.folders[0]?.path || OTHER;
      }
      permissionMode = o.permissionMode;
      loadState = "ready";
    } catch (e) {
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);

  async function focus(id: string) {
    await tick();
    document.getElementById(id)?.focus();
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    if (!cwd) {
      folderError = t("phone.new.folderRequired");
      focus(folder === OTHER ? "pn-path" : "pn-folder");
      return;
    }
    if (headless && !prompt.trim()) {
      promptError = t("phone.new.promptRequired");
      focus("pn-prompt");
      return;
    }
    if (headless && cli === "gemini") {
      failure = t("phone.new.geminiHeadless");
      return;
    }
    busy = true;
    try {
      const body = JSON.stringify({ cli, cwd, mode, prompt: prompt.trim(), permissionMode });
      const r = await call<{ session: SessionInfo }>("/api/sessions", { method: "POST", body });
      await goto(`/m/session?id=${r.session.id}`, { replaceState: true });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg === "This folder does not exist.") {
        folderError = msg;
        focus(folder === OTHER ? "pn-path" : "pn-folder");
      } else {
        failure = msg;
      }
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>{t("phone.newSession")} · OpenCompanion</title></svelte:head>

<header class="bar m-bar-tight">
  <a class="m-icon-btn m-back" href="/m" aria-label={t("phone.session.back")} title={t("phone.session.back")}>
    <CaretLeft size={20} aria-hidden="true" />
  </a>
  <h1 class="m-bar-title">{t("phone.newSession")}</h1>
</header>
<main class="content dense" id="phone-new-session">
  {#if loadState === "loading"}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("phone.new.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{tb(loadError)}</p>
    <button class="btn secondary block" type="button" onclick={load}>{t("phone.tryAgain")}</button>
  {:else if options}
    <p class="m-lead">
      {t("phone.new.lead")}
    </p>
    <form class="m-form" novalidate onsubmit={submit}>
      <fieldset>
        <legend>CLI</legend>
        {#if installed.length === 0}
          <p class="error">{t("phone.new.noCli")}</p>
        {/if}
        <div class="opts m-cli-opts">
          {#each options.clis as c (c.kind)}
            <label class="opt">
              <input type="radio" name="pn-cli" value={c.kind} bind:group={cli} disabled={!c.path} />
              <CliMark kind={c.kind} small />
              <span class="m-opt-text"><b>{c.label}</b><small class={c.path ? "mono" : ""}>{c.path ? (c.version ?? t("phone.new.versionUnknown")) : t("phone.new.notInstalled")}</small></span>
              <span class="check"><CheckCircle size={16} aria-hidden="true" /></span>
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="field">
        <label class="label" for="pn-folder">{t("phone.new.folder")}</label>
        <select
          class="select"
          id="pn-folder"
          bind:value={folder}
          onchange={() => (folderError = "")}
          aria-invalid={folderError && folder !== OTHER ? "true" : undefined}
          aria-describedby="pn-folder-note"
        >
          {#each options.folders as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
          <option value={OTHER}>{t("phone.new.otherFolder")}</option>
        </select>
        {#if folder === OTHER}
          <label class="sr-only" for="pn-path">{t("phone.new.path")}</label>
          <input
            class="input mono"
            id="pn-path"
            bind:value={typed}
            oninput={() => (folderError = "")}
            placeholder={t("phone.new.pathPlaceholder")}
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            aria-invalid={folderError ? "true" : undefined}
            aria-describedby="pn-folder-note"
          />
        {/if}
        {#if folderError}
          <p class="error" id="pn-folder-note">{folderError}</p>
        {:else}
          <p class="help" id="pn-folder-note">{t("phone.new.folderHelp")}</p>
        {/if}
      </div>

      <!-- Two radios drawn as a segmented control; the help under it follows the one picked. -->
      <fieldset aria-describedby="pn-mode-help">
        <legend>{t("phone.new.mode")}</legend>
        <div class="m-seg m-views m-radio-seg">
          <label>
            <input class="sr-only" type="radio" name="pn-mode" value="headless" bind:group={mode} />
            <ListBullets size={16} aria-hidden="true" />{t("phone.new.headless")}
          </label>
          <label>
            <input class="sr-only" type="radio" name="pn-mode" value="interactive" bind:group={mode} />
            <TerminalWindow size={16} aria-hidden="true" />{t("phone.new.interactive")}
          </label>
        </div>
        <p class="help" id="pn-mode-help">{headless ? t("phone.new.headlessHelp") : t("phone.new.interactiveHelp")}</p>
      </fieldset>

      <div class="field">
        <label class="label" for="pn-prompt">{headless ? t("phone.new.prompt") : t("phone.new.firstMessage")}</label>
        <textarea
          class="textarea"
          id="pn-prompt"
          bind:value={prompt}
          oninput={() => (promptError = "")}
          aria-invalid={promptError ? "true" : undefined}
          aria-describedby={promptError ? "pn-prompt-error" : undefined}
          placeholder={headless ? t("phone.new.promptPlaceholder", { cli: CLI_LABEL[cli] }) : t("phone.new.firstMessagePlaceholder", { cli: CLI_LABEL[cli] })}
        ></textarea>
        {#if promptError}<p class="error" id="pn-prompt-error">{promptError}</p>{/if}
      </div>

      <div class="field">
        <label class="label" for="pn-perm">{t("phone.new.permission")}</label>
        <select class="select" id="pn-perm" bind:value={permissionMode} aria-describedby="pn-perm-help">
          {#each MODES as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
        </select>
        <p class="help" id="pn-perm-help">{modeInfo.short} {t("phone.new.permissionDefault")}</p>
        {#if permissionMode === "bypass"}
          <p class="error" role="note">{t("phone.new.bypass", { cli: CLI_LABEL[cli] })}</p>
        {/if}
      </div>

      {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

      <button class="btn primary block" type="submit" id="btn-start-session" disabled={busy || installed.length === 0}>
        {#if busy}<SpinnerGap size={16} class="spin" aria-hidden="true" />{:else}<Play size={16} aria-hidden="true" />{/if}<span style="overflow:hidden;text-overflow:ellipsis">{busy ? t("phone.new.starting") : label}</span>
      </button>
    </form>
  {/if}
</main>

<style>
  .m-back {
    margin-left: -8px;
  }
  .m-bar-title {
    flex: 1;
    min-width: 0;
    font-size: 17px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-lead {
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.45;
  }
  /* Four CLIs fit two by two; a long version string ends in an ellipsis. */
  .m-cli-opts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .m-cli-opts .opt {
    min-height: 44px;
    padding: 8px 10px;
    gap: 8px;
  }
  .m-cli-opts .opt:has(input:checked) {
    padding: 7px 9px;
  }
  .m-opt-text {
    min-width: 0;
  }
  .m-opt-text b {
    font-size: 13px;
  }
  .m-opt-text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-radio-seg label {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 40px;
    border-radius: var(--r-sm);
    color: var(--ink-2);
    font-weight: 700;
    font-size: 13px;
    cursor: pointer;
  }
  .m-radio-seg label:has(input:checked) {
    background: var(--surface);
    color: var(--ink);
  }
  .m-radio-seg label:has(input:checked) :global(svg) {
    color: var(--forest);
  }
  .m-radio-seg label:has(input:focus-visible) {
    outline: 2px solid var(--forest);
    outline-offset: 2px;
  }
  .m-radio-seg + .help {
    margin-top: 6px;
  }
</style>
