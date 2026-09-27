<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import Copy from "phosphor-svelte/lib/Copy";
  import GearSix from "phosphor-svelte/lib/GearSix";
  import X from "phosphor-svelte/lib/X";
  import { errorText, type CliInstall, type CliKind } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import { shortPath } from "$lib/format";
  import SettingsHead from "$lib/SettingsHead.svelte";
  import { t, tb } from "$lib/i18n.svelte";
  import { app, refreshClis, saveSettings, showToast } from "$lib/store.svelte";

  // null: no headless adapter yet, shown as translated text.
  const HEADLESS: Record<CliKind, string | null> = {
    claude: "claude -p --output-format stream-json",
    codex: "codex exec --json -C <folder>",
    opencode: "opencode run --format json --dir <folder>",
    gemini: null,
    ccs: "ccs [profile] -p --output-format stream-json",
    pi: "pi --mode json",
  };
  // Official npm packages. OpenCompanion never runs these itself (PRD FR-03).
  const INSTALL: Record<CliKind, string> = {
    claude: "npm install -g @anthropic-ai/claude-code",
    codex: "npm install -g @openai/codex",
    opencode: "npm install -g opencode-ai",
    gemini: "npm install -g @google/gemini-cli",
    ccs: "npm install -g @kaitranntt/ccs",
    pi: "npm install -g --ignore-scripts @earendil-works/pi-coding-agent",
  };

  let configOpen = $state(false);
  let configCli = $state<CliInstall | null>(null);
  let pathValue = $state("");
  let argsValue = $state("");
  let configError = $state("");
  let savingConfig = $state(false);

  const planner = $derived(
    app.settings?.plannerSource === "api"
      ? null
      : (app.settings?.chatCli ?? app.clis.find((c) => c.path && c.kind !== "gemini")?.kind ?? null),
  );
  const foundCount = $derived(app.clis.filter((c) => c.path).length);
  // The --version flag sits in code type between the two halves.
  const sub = $derived(t("work.clis.sub").split("{flag}"));

  async function rescan() {
    await refreshClis();
    if (app.clisState === "ready") {
      const missing = app.clis.filter((c) => !c.path).map((c) => c.label);
      const counts = { found: foundCount, total: app.clis.length };
      showToast(missing.length ? t("work.clis.scanDoneMissing", { ...counts, missing: missing.join(", ") }) : t("work.clis.scanDone", counts));
    }
  }

  async function usePlanner(kind: CliKind) {
    if (!app.settings) return;
    try {
      await saveSettings({ ...app.settings, chatCli: kind, plannerSource: "cli" });
      showToast(t("work.clis.plannerSet", { cli: app.clis.find((c) => c.kind === kind)?.label ?? kind }));
    } catch (e) {
      showToast(tb(errorText(e)));
    }
  }

  function configure(c: CliInstall) {
    configCli = c;
    pathValue = app.settings?.cliPaths[c.kind] ?? "";
    argsValue = app.settings?.cliArgs[c.kind] ?? "";
    configError = "";
    configOpen = true;
  }

  async function pickFile() {
    const picked = await openDialog({ multiple: false, directory: false, title: t("work.clis.pickTitle"), filters: [{ name: t("work.clis.pickFilter"), extensions: ["exe", "cmd", "bat"] }] });
    if (typeof picked === "string") pathValue = picked;
  }

  async function saveConfig(e: SubmitEvent) {
    e.preventDefault();
    if (!app.settings || !configCli) return;
    const p = pathValue.trim();
    if (p && !/^([A-Za-z]:[\\/]|\\\\|\/).+/.test(p)) {
      configError = t("work.clis.pathInvalid");
      return;
    }
    savingConfig = true;
    try {
      const cliPaths = { ...app.settings.cliPaths };
      const cliArgs = { ...app.settings.cliArgs };
      if (p) cliPaths[configCli.kind] = p;
      else delete cliPaths[configCli.kind];
      if (argsValue.trim()) cliArgs[configCli.kind] = argsValue.trim();
      else delete cliArgs[configCli.kind];
      await saveSettings({ ...app.settings, cliPaths, cliArgs });
      configOpen = false;
      await refreshClis();
      showToast(t("work.clis.saved", { cli: configCli.label }));
    } catch (err) {
      configError = errorText(err);
    } finally {
      savingConfig = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      showToast(t("work.clis.copied", { text }));
    } catch {
      showToast(t("work.clis.copyFailed"));
    }
  }
</script>

<svelte:head><title>{t("work.clis.pageTitle")}</title></svelte:head>

<main class="main" id="clis-main">
  <SettingsHead>
    <button class="btn secondary" type="button" id="btn-rescan" disabled={app.checkingClis} onclick={rescan}>
      <ArrowClockwise size={16} aria-hidden="true" /><span>{app.checkingClis ? t("work.scanning") : t("work.rescan")}</span>
    </button>
  </SettingsHead>
  <h2 class="sr-only">{t("work.clis.heading")}</h2>
  <p class="note">{sub[0]}<span class="mono">--version</span>{sub[1]}</p>

  {#if app.clisState === "loading"}
    <p class="hint" role="status">{t("work.clis.checking")}</p>
  {:else if app.clisState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("work.clis.loadError")}</h2>
      <p>{tb(app.clisError)}</p>
      <button class="btn secondary" type="button" onclick={rescan}>{t("work.tryAgain")}</button>
    </div>
  {:else}
    <div class="table-wrap">
      <table id="cli-table" aria-busy={app.checkingClis}>
        <caption class="sr-only">{t("work.clis.caption")}</caption>
        <thead>
          <tr>
            <th scope="col" style="width:32%">{t("work.clis.colCli")}</th>
            <th scope="col">{t("work.clis.colVersion")}</th>
            <th scope="col" style="width:28%">{t("work.clis.colHeadless")}</th>
            <th scope="col">{t("work.clis.colAdapter")}</th>
            <th scope="col"><span class="sr-only">{t("work.clis.colActions")}</span></th>
          </tr>
        </thead>
        <tbody>
          {#each app.clis as c (c.kind)}
            {#if c.path}
              <tr>
                <td>
                  <div class="name">
                    <CliMark kind={c.kind} />
                    <span><b>{c.label}</b><span class="mono" title={c.path}>{shortPath(c.path)}</span>
                      {#if app.settings?.cliArgs[c.kind]}<span class="mono">{t("work.clis.extraArgs", { args: app.settings.cliArgs[c.kind] })}</span>{/if}
                    </span>
                  </div>
                </td>
                <td class="mono">{c.version ?? t("work.clis.versionUnknown")}</td>
                <td class="mono">{HEADLESS[c.kind] ?? t("work.clis.headlessUnsupported")}</td>
                <td>
                  {#if c.error}
                    <span class="chip err" title={tb(c.error)}>{t("work.clis.versionUnreadable")}</span>
                  {:else if c.tested}
                    <span class="ok"><CheckCircle size={16} aria-hidden="true" />{t("work.clis.tested")}</span>
                  {:else}
                    <span class="chip wait" title={t("work.clis.untestedHint")}>{t("work.clis.untested")}</span>
                  {/if}
                </td>
                <td class="acts">
                  {#if c.kind !== "gemini"}
                    {#if planner === c.kind}
                      <span class="planner-label">{t("work.clis.chatPlanner")}</span>
                    {:else}
                      <button class="btn secondary sm" type="button" onclick={() => usePlanner(c.kind)}>{t("work.clis.usePlanner")}</button>
                    {/if}
                  {/if}
                  <button class="icon-btn" type="button" aria-label={t("work.clis.settingsFor", { cli: c.label })} title={t("work.clis.pathAndArgs")} onclick={() => configure(c)}><GearSix size={18} aria-hidden="true" /></button>
                </td>
              </tr>
            {:else}
              <tr class="missing">
                <td>
                  <div class="name">
                    <CliMark kind={c.kind} />
                    <span><b>{c.label}</b><span class="meta">{c.error ?? t("work.clis.notOnPath")}</span></span>
                  </div>
                </td>
                <td colspan="3">
                  <div class="install">
                    <code>{INSTALL[c.kind]}</code>
                    <button class="btn secondary sm" type="button" onclick={() => copy(INSTALL[c.kind])}><Copy size={16} aria-hidden="true" />{t("work.copyCommand")}</button>
                  </div>
                </td>
                <td class="acts">
                  <button class="icon-btn" type="button" aria-label={t("work.clis.setPathFor", { cli: c.label })} title={t("work.clis.setPath")} onclick={() => configure(c)}><GearSix size={18} aria-hidden="true" /></button>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">{t("work.clis.note")}</p>
  {/if}
</main>

<Dialog bind:open={configOpen} labelledby="cfg-title">
  {#if configCli}
    <form class="d-body" novalidate onsubmit={saveConfig}>
      <div class="row">
        <h2 id="cfg-title" class="grow">{t("work.clis.settingsTitle", { cli: configCli.label })}</h2>
        <button class="icon-btn" type="button" aria-label={t("work.close")} onclick={() => (configOpen = false)}><X size={18} aria-hidden="true" /></button>
      </div>
      <div class="field">
        <label class="label" for="cfg-path">{t("work.clis.pathLabel")}</label>
        <div class="row" style="gap:10px">
          <input class="input mono grow" id="cfg-path" bind:value={pathValue} oninput={() => (configError = "")} placeholder={t("work.clis.pathPlaceholder")} spellcheck="false" aria-describedby="cfg-path-help" />
          <button class="btn secondary" type="button" onclick={pickFile}>{t("work.clis.browse")}</button>
        </div>
        <p class="help" id="cfg-path-help">{t("work.clis.pathHelp")}</p>
      </div>
      <div class="field">
        <label class="label" for="cfg-args">{t("work.clis.argsLabel")}</label>
        <input class="input mono" id="cfg-args" bind:value={argsValue} placeholder={t(configCli.kind === "ccs" ? "work.clis.argsPlaceholderCcs" : "work.clis.argsPlaceholder")} spellcheck="false" aria-describedby="cfg-args-help" />
        <p class="help" id="cfg-args-help">{t(configCli.kind === "ccs" ? "work.clis.argsHelpCcs" : "work.clis.argsHelp")}</p>
      </div>
      {#if configError}<p class="err-text" role="alert" style="margin:0">{tb(configError)}</p>{/if}
      <div class="d-foot">
        <span class="meta grow">{t("work.escToClose")}</span>
        <button class="btn secondary" type="button" onclick={() => (configOpen = false)}>{t("work.cancel")}</button>
        <button class="btn primary" type="submit" disabled={savingConfig}>{t("work.clis.saveRescan")}</button>
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .table-wrap {
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--surface);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    min-width: 860px;
  }
  th {
    text-align: left;
    font-weight: 700;
    color: var(--ink-2);
    background: var(--surface-2);
    padding: 10px 16px;
  }
  td {
    padding: 14px 16px;
    border-top: 1px solid var(--line);
    vertical-align: middle;
  }
  td .name {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  td .name b {
    display: block;
    font-size: 14px;
  }
  td .name .mono,
  td .name .meta {
    display: block;
    font-size: 12px;
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  td.mono {
    font-size: 12px;
  }
  td.acts {
    text-align: right;
    white-space: nowrap;
  }
  td.acts > * {
    vertical-align: middle;
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .ok :global(svg) {
    color: var(--forest);
  }
  .planner-label {
    font-weight: 700;
    color: var(--forest);
    margin-right: 6px;
  }
  tr.missing td {
    background: var(--bg);
  }
  .install {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .install code {
    flex: 1;
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: var(--term-bg);
    color: var(--term-text);
    font: 13px var(--font-mono);
  }
  .note {
    max-width: 720px;
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.45;
  }
</style>
