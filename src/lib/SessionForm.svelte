<script lang="ts" module>
  import type { CliKind as Kind, Mode as RunMode, PermMode } from "./api";

  export interface SessionValues {
    cli: Kind;
    cwd: string;
    mode: RunMode;
    prompt: string;
    permissionMode: PermMode;
  }
</script>

<script lang="ts">
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import Play from "phosphor-svelte/lib/Play";
  import X from "phosphor-svelte/lib/X";
  import { untrack } from "svelte";
  import { api, errorText, type CliKind, type Mode, type PermMode as Perm } from "./api";
  import CliMark from "./CliMark.svelte";
  import FolderField from "./FolderField.svelte";
  import { CLI_LABEL, MODES, folderName } from "./format";
  import { t, tb } from "./i18n.svelte";
  import { app } from "./store.svelte";

  let {
    title,
    initial,
    submitText,
    onsubmit,
    oncancel,
    idPrefix = "ns",
  }: {
    title: string;
    initial: Partial<SessionValues>;
    submitText?: (v: SessionValues) => string;
    onsubmit: (v: SessionValues) => Promise<void>;
    oncancel: () => void;
    idPrefix?: string;
  } = $props();

  const installed = $derived(app.clis.filter((c) => c.path));
  const firstInstalled = (): CliKind => app.clis.find((c) => c.path && c.kind !== "gemini")?.kind ?? "claude";

  // The form starts from `initial` once; the dialog remounts it every time it opens.
  const start = untrack(() => ({ ...initial }));
  let cli = $state<CliKind>(start.cli ?? firstInstalled());
  let cwd = $state(start.cwd ?? "");
  let mode = $state<Mode>(start.mode ?? "interactive");
  let prompt = $state(start.prompt ?? "");
  let permissionMode = $state<Perm>(start.permissionMode ?? app.settings?.permissionMode ?? "ask");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);
  let promptEl: HTMLTextAreaElement | undefined = $state();

  const values = $derived({ cli, cwd: cwd.trim(), mode, prompt: prompt.trim(), permissionMode });
  const modeInfo = $derived(MODES.find((m) => m.id === permissionMode) ?? MODES[0]);
  const label = $derived(
    submitText ? submitText(values) : t("shell.form.start", { cli: CLI_LABEL[cli], folder: values.cwd ? folderName(values.cwd) : "…" }),
  );
  const headless = $derived(mode === "headless");

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    if (!values.cwd || !(await api.folderExists(values.cwd))) {
      folderError = values.cwd ? t("shell.form.folderMissing") : t("shell.form.folderEmpty");
      document.getElementById(`${idPrefix}-folder`)?.focus();
      return;
    }
    if (headless && !values.prompt) {
      promptError = t("shell.form.promptNeeded");
      promptEl?.focus();
      return;
    }
    if (headless && cli === "gemini") {
      failure = t("shell.form.geminiHeadless");
      return;
    }
    busy = true;
    try {
      await onsubmit(values);
    } catch (err) {
      failure = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="d-body" novalidate onsubmit={submit}>
  <div class="row">
    <h2 id="{idPrefix}-title" class="grow">{title}</h2>
    <button class="icon-btn" type="button" aria-label={t("shell.close")} onclick={oncancel}><X size={18} aria-hidden="true" /></button>
  </div>

  <fieldset class="field bare">
    <legend class="label">{t("shell.form.cli")}</legend>
    {#if app.clisState === "loading"}
      <p class="help">{t("shell.form.checking")}</p>
    {:else if installed.length === 0}
      <p class="error">{t("shell.form.noCli")}</p>
    {/if}
    <div class="opts">
      {#each app.clis as c (c.kind)}
        <label class="opt">
          <input type="radio" name="{idPrefix}-cli" value={c.kind} bind:group={cli} disabled={!c.path} />
          <CliMark kind={c.kind} />
          <span><b>{c.label}</b><small class={c.path ? "mono" : ""}>{c.path ? (c.version ?? t("shell.form.versionUnknown")) : t("shell.form.notInstalled")}</small></span>
          <span class="check"><CheckCircle size={18} aria-hidden="true" /></span>
        </label>
      {/each}
    </div>
  </fieldset>

  <FolderField id="{idPrefix}-folder" bind:value={cwd} error={folderError} oninput={() => (folderError = "")} />

  <fieldset class="field bare">
    <legend class="label">{t("shell.form.mode")}</legend>
    <div class="opts">
      <label class="opt stack">
        <input type="radio" name="{idPrefix}-mode" value="interactive" bind:group={mode} />
        <b>{t("shell.form.interactive")}</b>
        <p>{t("shell.form.interactiveHelp")}</p>
      </label>
      <label class="opt stack">
        <input type="radio" name="{idPrefix}-mode" value="headless" bind:group={mode} />
        <b>{t("shell.form.headless")}</b>
        <p>{t("shell.form.headlessHelp")}</p>
      </label>
    </div>
  </fieldset>

  <div class="field">
    <label class="label" for="{idPrefix}-prompt">{headless ? t("shell.form.prompt") : t("shell.form.firstMessage")}</label>
    <textarea
      class="textarea"
      id="{idPrefix}-prompt"
      bind:this={promptEl}
      bind:value={prompt}
      oninput={() => (promptError = "")}
      aria-invalid={promptError ? "true" : undefined}
      aria-describedby={promptError ? `${idPrefix}-prompt-error` : undefined}
      placeholder={headless
        ? t("shell.form.promptPlaceholder", { cli: CLI_LABEL[cli] })
        : t("shell.form.messagePlaceholder", { cli: CLI_LABEL[cli] })}
    ></textarea>
    {#if promptError}<p class="error" id="{idPrefix}-prompt-error">{promptError}</p>{/if}
  </div>

  <div class="field">
    <label class="label" for="{idPrefix}-perm">{t("shell.form.permMode")}</label>
    <select class="select" id="{idPrefix}-perm" bind:value={permissionMode} aria-describedby="{idPrefix}-perm-help">
      {#each MODES as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
    </select>
    <p class="help" id="{idPrefix}-perm-help">{modeInfo.short} {t("shell.form.permDefault")}</p>
    {#if permissionMode === "bypass"}
      <p class="error" role="note">{t("shell.form.bypassWarning", { cli: CLI_LABEL[cli] })}</p>
    {/if}
  </div>

  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  <div class="d-foot">
    <span class="meta grow">{t("shell.form.escHint")}</span>
    <button class="btn secondary" type="button" onclick={oncancel}>{t("shell.form.cancel")}</button>
    <button class="btn primary" type="submit" disabled={busy || installed.length === 0}>
      <Play size={16} aria-hidden="true" /><span>{busy ? t("shell.form.starting") : label}</span>
    </button>
  </div>
</form>

<style>
  .bare {
    border: 0;
    padding: 0;
    margin: 0;
  }
  .bare legend {
    padding: 0;
    margin-bottom: 8px;
    font-weight: 800;
  }
  .opt input {
    margin: 0;
  }
</style>
