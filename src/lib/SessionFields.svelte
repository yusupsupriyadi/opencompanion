<script lang="ts" module>
  import { api as backend, type CliKind as Kind, type Mode as RunMode, type PermMode as Perm } from "./api";
  import { t as tr } from "./i18n.svelte";

  export interface SessionValues {
    cli: Kind;
    cwd: string;
    mode: RunMode;
    prompt: string;
    permissionMode: Perm;
  }

  export interface SessionProblems {
    folderError?: string;
    promptError?: string;
    failure?: string;
  }

  /** What New session and the automation form check before they send: an empty object when nothing. */
  export async function checkSession(v: SessionValues): Promise<SessionProblems> {
    if (!v.cwd || !(await backend.folderExists(v.cwd))) {
      return { folderError: v.cwd ? tr("shell.form.folderMissing") : tr("shell.form.folderEmpty") };
    }
    if (v.mode === "headless" && !v.prompt) return { promptError: tr("shell.form.promptNeeded") };
    if (v.mode === "headless" && v.cli === "gemini") return { failure: tr("shell.form.geminiHeadless") };
    return {};
  }
</script>

<script lang="ts">
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import type { CliKind, Mode, PermMode } from "./api";
  import CliMark from "./CliMark.svelte";
  import FolderField from "./FolderField.svelte";
  import { CLI_LABEL, MODES } from "./format";
  import { t } from "./i18n.svelte";
  import { app } from "./store.svelte";

  // The fields New session and the automation form share: CLI, folder, mode, prompt, permission mode. `blank` offers a
  // blank terminal beside the CLIs, which New session does and an automation, with no prompt to run, does not.
  let {
    cli = $bindable(),
    cwd = $bindable(),
    mode = $bindable(),
    prompt = $bindable(),
    permissionMode = $bindable(),
    folderError = $bindable(""),
    promptError = $bindable(""),
    promptEl = $bindable(),
    idPrefix,
    blank = false,
  }: {
    cli: CliKind;
    cwd: string;
    mode: Mode;
    prompt: string;
    permissionMode: PermMode;
    folderError?: string;
    promptError?: string;
    promptEl?: HTMLTextAreaElement;
    idPrefix: string;
    blank?: boolean;
  } = $props();

  const installed = $derived(app.clis.filter((c) => c.path));
  const modeInfo = $derived(MODES.find((m) => m.id === permissionMode) ?? MODES[0]);
  const headless = $derived(mode === "headless");
  // A blank terminal is only a shell: no mode, no first message, no permissions to ask about.
  const shellOnly = $derived(cli === "terminal");
</script>

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
    {#if blank}
      <label class="opt" id="{idPrefix}-cli-terminal">
        <input type="radio" name="{idPrefix}-cli" value="terminal" bind:group={cli} />
        <CliMark kind="terminal" />
        <span><b>{t("shell.form.blank")}</b><small>{t("shell.form.blankHelp")}</small></span>
        <span class="check"><CheckCircle size={18} aria-hidden="true" /></span>
      </label>
    {/if}
  </div>
</fieldset>

<FolderField id="{idPrefix}-folder" bind:value={cwd} error={folderError} oninput={() => (folderError = "")} />

{#if !shellOnly}
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
{/if}

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
