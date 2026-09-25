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
  import { app, refreshClis, saveSettings, showToast } from "$lib/store.svelte";

  const HEADLESS: Record<CliKind, string> = {
    claude: "claude -p --output-format stream-json",
    codex: "codex exec --json -C <folder>",
    opencode: "opencode run --format json --dir <folder>",
    gemini: "Not supported yet",
  };
  // Official npm packages. AI Remote never runs these itself (PRD FR-03).
  const INSTALL: Record<CliKind, string> = {
    claude: "npm install -g @anthropic-ai/claude-code",
    codex: "npm install -g @openai/codex",
    opencode: "npm install -g opencode-ai",
    gemini: "npm install -g @google/gemini-cli",
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

  async function rescan() {
    await refreshClis();
    if (app.clisState === "ready") {
      const missing = app.clis.filter((c) => !c.path).map((c) => c.label);
      showToast(`Scan finished: ${foundCount} of ${app.clis.length} found${missing.length ? `, ${missing.join(", ")} not found` : ""}.`);
    }
  }

  async function usePlanner(kind: CliKind) {
    if (!app.settings) return;
    try {
      await saveSettings({ ...app.settings, chatCli: kind, plannerSource: "cli" });
      showToast(`${app.clis.find((c) => c.kind === kind)?.label} is now the Chat planner.`);
    } catch (e) {
      showToast(errorText(e));
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
    const picked = await openDialog({ multiple: false, directory: false, title: "Choose the CLI executable", filters: [{ name: "Programs", extensions: ["exe", "cmd", "bat"] }] });
    if (typeof picked === "string") pathValue = picked;
  }

  async function saveConfig(e: SubmitEvent) {
    e.preventDefault();
    if (!app.settings || !configCli) return;
    const p = pathValue.trim();
    if (p && !/^([A-Za-z]:[\\/]|\\\\|\/).+/.test(p)) {
      configError = "Enter the full path to the executable, or leave it empty to use PATH.";
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
      showToast(`Saved ${configCli.label} settings.`);
    } catch (err) {
      configError = errorText(err);
    } finally {
      savingConfig = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      showToast(`Copied ${text}`);
    } catch {
      showToast("Could not copy. Select the text and copy it by hand.");
    }
  }
</script>

<svelte:head><title>CLIs · AI Remote</title></svelte:head>

<main class="main" id="clis-main">
  <header class="page-head">
    <div class="grow">
      <h1>CLIs</h1>
      <p class="sub">Found on your PATH and in common install folders. Versions come from each CLI's own <span class="mono">--version</span> output.</p>
    </div>
    <button class="btn secondary" type="button" id="btn-rescan" disabled={app.checkingClis} onclick={rescan}>
      <ArrowClockwise size={16} aria-hidden="true" /><span>{app.checkingClis ? "Scanning…" : "Rescan"}</span>
    </button>
  </header>

  {#if app.clisState === "loading"}
    <p class="hint" role="status">Checking PATH and common install folders…</p>
  {:else if app.clisState === "error"}
    <div class="state-box" role="alert">
      <h2>CLIs could not be checked</h2>
      <p>{app.clisError}</p>
      <button class="btn secondary" type="button" onclick={rescan}>Try again</button>
    </div>
  {:else}
    <div class="table-wrap">
      <table id="cli-table" aria-busy={app.checkingClis}>
        <caption class="sr-only">Coding CLIs found on this computer</caption>
        <thead>
          <tr>
            <th scope="col" style="width:32%">CLI</th>
            <th scope="col">Version</th>
            <th scope="col" style="width:28%">Headless command</th>
            <th scope="col">Adapter</th>
            <th scope="col"><span class="sr-only">Actions</span></th>
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
                      {#if app.settings?.cliArgs[c.kind]}<span class="mono">extra: {app.settings.cliArgs[c.kind]}</span>{/if}
                    </span>
                  </div>
                </td>
                <td class="mono">{c.version ?? "unknown"}</td>
                <td class="mono">{HEADLESS[c.kind]}</td>
                <td>
                  {#if c.error}
                    <span class="chip err" title={c.error}>Can't read version</span>
                  {:else if c.tested}
                    <span class="ok"><CheckCircle size={16} aria-hidden="true" />Tested</span>
                  {:else}
                    <span class="chip wait" title="The adapter has not been tested with this version yet. Headless mode still works, but events may look different.">Untested version</span>
                  {/if}
                </td>
                <td class="acts">
                  {#if c.kind !== "gemini"}
                    {#if planner === c.kind}
                      <span class="planner-label">Chat planner</span>
                    {:else}
                      <button class="btn secondary sm" type="button" onclick={() => usePlanner(c.kind)}>Use as planner</button>
                    {/if}
                  {/if}
                  <button class="icon-btn" type="button" aria-label="Settings for {c.label}" title="Path and extra arguments" onclick={() => configure(c)}><GearSix size={18} aria-hidden="true" /></button>
                </td>
              </tr>
            {:else}
              <tr class="missing">
                <td>
                  <div class="name">
                    <CliMark kind={c.kind} />
                    <span><b>{c.label}</b><span class="meta">{c.error ?? "Not found on PATH"}</span></span>
                  </div>
                </td>
                <td colspan="3">
                  <div class="install">
                    <code>{INSTALL[c.kind]}</code>
                    <button class="btn secondary sm" type="button" onclick={() => copy(INSTALL[c.kind])}><Copy size={16} aria-hidden="true" />Copy command</button>
                  </div>
                </td>
                <td class="acts">
                  <button class="icon-btn" type="button" aria-label="Set a path for {c.label}" title="Set a path" onclick={() => configure(c)}><GearSix size={18} aria-hidden="true" /></button>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">AI Remote never installs a CLI for you. Copy the command, run it in your own terminal, then press Rescan. Aider, Qwen Code and other CLIs are planned after the first release.</p>
  {/if}
</main>

<Dialog bind:open={configOpen} labelledby="cfg-title">
  {#if configCli}
    <form class="d-body" novalidate onsubmit={saveConfig}>
      <div class="row">
        <h2 id="cfg-title" class="grow">{configCli.label} settings</h2>
        <button class="icon-btn" type="button" aria-label="Close" onclick={() => (configOpen = false)}><X size={18} aria-hidden="true" /></button>
      </div>
      <div class="field">
        <label class="label" for="cfg-path">Executable path</label>
        <div class="row" style="gap:10px">
          <input class="input mono grow" id="cfg-path" bind:value={pathValue} oninput={() => (configError = "")} placeholder="Empty: use the one found on PATH" spellcheck="false" aria-describedby="cfg-path-help" />
          <button class="btn secondary" type="button" onclick={pickFile}>Browse</button>
        </div>
        <p class="help" id="cfg-path-help">AI Remote runs it once with --version to read the version. Nothing else is executed until you start a session.</p>
      </div>
      <div class="field">
        <label class="label" for="cfg-args">Extra arguments</label>
        <input class="input mono" id="cfg-args" bind:value={argsValue} placeholder="For example --pure" spellcheck="false" aria-describedby="cfg-args-help" />
        <p class="help" id="cfg-args-help">Added to every session this CLI runs in AI Remote, separated by spaces. Quotes are not supported.</p>
      </div>
      {#if configError}<p class="err-text" role="alert" style="margin:0">{configError}</p>{/if}
      <div class="d-foot">
        <span class="meta grow">Esc to close</span>
        <button class="btn secondary" type="button" onclick={() => (configOpen = false)}>Cancel</button>
        <button class="btn primary" type="submit" disabled={savingConfig}>Save and rescan</button>
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
