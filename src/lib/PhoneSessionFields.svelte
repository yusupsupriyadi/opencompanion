<script lang="ts" module>
  /** The folder select's "Another folder…" choice, which opens a path field. */
  export const OTHER_FOLDER = "__other";
</script>

<script lang="ts">
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import ListBullets from "phosphor-svelte/lib/ListBullets";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import type { CliKind, Mode, PermMode } from "./api";
  import CliMark from "./CliMark.svelte";
  import { CLI_LABEL, MODES, shortPath } from "./format";
  import { t } from "./i18n.svelte";
  import type { PhoneOptions } from "./phone.svelte";

  // The fields the phone's New session and automation forms share: CLI, folder, mode, prompt and
  // permission mode, from the options the desktop sends.
  let {
    options,
    cli = $bindable(),
    folder = $bindable(),
    typed = $bindable(),
    mode = $bindable(),
    prompt = $bindable(),
    permissionMode = $bindable(),
    folderError = $bindable(""),
    promptError = $bindable(""),
    idPrefix,
  }: {
    options: PhoneOptions;
    cli: CliKind;
    folder: string;
    typed: string;
    mode: Mode;
    prompt: string;
    permissionMode: PermMode;
    folderError?: string;
    promptError?: string;
    idPrefix: string;
  } = $props();

  const installed = $derived(options.clis.filter((c) => c.path));
  const headless = $derived(mode === "headless");
  const modeInfo = $derived(MODES.find((m) => m.id === permissionMode) ?? MODES[0]);
</script>

  <fieldset>
    <legend>CLI</legend>
    {#if installed.length === 0}
      <p class="error">{t("phone.new.noCli")}</p>
    {/if}
    <div class="opts m-cli-opts">
      {#each options.clis as c (c.kind)}
        <label class="opt">
          <input type="radio" name="{idPrefix}-cli" value={c.kind} bind:group={cli} disabled={!c.path} />
          <CliMark kind={c.kind} small />
          <span class="m-opt-text"><b>{c.label}</b><small class={c.path ? "mono" : ""}>{c.path ? (c.version ?? t("phone.new.versionUnknown")) : t("phone.new.notInstalled")}</small></span>
          <span class="check"><CheckCircle size={16} aria-hidden="true" /></span>
        </label>
      {/each}
    </div>
  </fieldset>

  <div class="field">
    <label class="label" for="{idPrefix}-folder">{t("phone.new.folder")}</label>
    <select
      class="select"
      id="{idPrefix}-folder"
      bind:value={folder}
      onchange={() => (folderError = "")}
      aria-invalid={folderError && folder !== OTHER_FOLDER ? "true" : undefined}
      aria-describedby="{idPrefix}-folder-note"
    >
      {#each options.folders as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
      <option value={OTHER_FOLDER}>{t("phone.new.otherFolder")}</option>
    </select>
    {#if folder === OTHER_FOLDER}
      <label class="sr-only" for="{idPrefix}-path">{t("phone.new.path")}</label>
      <input
        class="input mono"
        id="{idPrefix}-path"
        bind:value={typed}
        oninput={() => (folderError = "")}
        placeholder={t("phone.new.pathPlaceholder")}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        aria-invalid={folderError ? "true" : undefined}
        aria-describedby="{idPrefix}-folder-note"
      />
    {/if}
    {#if folderError}
      <p class="error" id="{idPrefix}-folder-note">{folderError}</p>
    {:else}
      <p class="help" id="{idPrefix}-folder-note">{t("phone.new.folderHelp")}</p>
    {/if}
  </div>

  <!-- Two radios drawn as a segmented control; the help under it follows the one picked. -->
  <fieldset aria-describedby="{idPrefix}-mode-help">
    <legend>{t("phone.new.mode")}</legend>
    <div class="m-seg m-views m-radio-seg">
      <label>
        <input class="sr-only" type="radio" name="{idPrefix}-mode" value="headless" bind:group={mode} />
        <ListBullets size={16} aria-hidden="true" />{t("phone.new.headless")}
      </label>
      <label>
        <input class="sr-only" type="radio" name="{idPrefix}-mode" value="interactive" bind:group={mode} />
        <TerminalWindow size={16} aria-hidden="true" />{t("phone.new.interactive")}
      </label>
    </div>
    <p class="help" id="{idPrefix}-mode-help">{headless ? t("phone.new.headlessHelp") : t("phone.new.interactiveHelp")}</p>
  </fieldset>

  <div class="field">
    <label class="label" for="{idPrefix}-prompt">{headless ? t("phone.new.prompt") : t("phone.new.firstMessage")}</label>
    <textarea
      class="textarea"
      id="{idPrefix}-prompt"
      bind:value={prompt}
      oninput={() => (promptError = "")}
      aria-invalid={promptError ? "true" : undefined}
      aria-describedby={promptError ? "{idPrefix}-prompt-error" : undefined}
      placeholder={headless ? t("phone.new.promptPlaceholder", { cli: CLI_LABEL[cli] }) : t("phone.new.firstMessagePlaceholder", { cli: CLI_LABEL[cli] })}
    ></textarea>
    {#if promptError}<p class="error" id="{idPrefix}-prompt-error">{promptError}</p>{/if}
  </div>

  <div class="field">
    <label class="label" for="{idPrefix}-perm">{t("phone.new.permission")}</label>
    <select class="select" id="{idPrefix}-perm" bind:value={permissionMode} aria-describedby="{idPrefix}-perm-help">
      {#each MODES as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
    </select>
    <p class="help" id="{idPrefix}-perm-help">{modeInfo.short} {t("phone.new.permissionDefault")}</p>
    {#if permissionMode === "bypass"}
      <p class="error" role="note">{t("phone.new.bypass", { cli: CLI_LABEL[cli] })}</p>
    {/if}
  </div>

<style>
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
