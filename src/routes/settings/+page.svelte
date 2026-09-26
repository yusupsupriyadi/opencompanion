<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import Copy from "phosphor-svelte/lib/Copy";
  import DeviceMobile from "phosphor-svelte/lib/DeviceMobile";
  import Warning from "phosphor-svelte/lib/Warning";
  import QRCode from "qrcode";
  import { onMount } from "svelte";
  import {
    api,
    errorText,
    type AppInfo,
    type CliKind,
    type Device,
    type NotifyRule,
    type Pairing,
    type PermMode,
    type PlannerApi,
    type ProjectFolder,
    type Settings,
  } from "$lib/api";
  import { MODES, TEXT_SIZES, ago, folderName, modeLabel, shortPath } from "$lib/format";
  import { app, loadSettings, refreshSessions, saveSettings, showToast } from "$lib/store.svelte";

  let settings = $state<Settings | null>(null);
  let loadError = $state("");
  let pairing = $state<Pairing | null>(null);
  let qrSvg = $state("");
  let pairError = $state("");
  let devices = $state<Device[]>([]);
  let info = $state<AppInfo | null>(null);
  let switching = $state(false);
  let clearStep = $state(false);
  let portDraft = $state(8765);
  let now = $state(Date.now());

  const on = $derived(Boolean(settings?.companionEnabled && app.companion?.running));
  const address = $derived(app.companion?.address ? `http://${app.companion.address}:${app.companion.port}` : "");
  const left = $derived(pairing ? Math.max(0, Math.round((pairing.expiresAt - now) / 1000)) : 0);
  const planners = $derived(app.clis.filter((c) => c.path && c.kind !== "gemini"));
  const usingApi = $derived(settings?.plannerSource === "api");

  async function pickPlanner(el: HTMLSelectElement) {
    const value = el.value;
    let ok;
    if (value === "api") {
      const done = settings?.plannerApi.baseUrl && settings.plannerApi.model;
      ok = await save({ plannerSource: "api" }, done ? "Chat planner saved." : "Chat planner saved. Add the provider's base URL and model below.");
    } else {
      ok = await save({ plannerSource: "cli", chatCli: value as Settings["chatCli"] }, "Chat planner saved.");
    }
    if (!ok) el.value = usingApi ? "api" : (settings?.chatCli ?? planners[0]?.kind ?? "");
  }

  // The provider fields are saved together, on Save provider, so a half-typed URL is never used.
  let apiDraft = $state<PlannerApi>({ baseUrl: "", model: "", apiKey: "" });
  let apiErrors = $state<{ baseUrl?: string; model?: string }>({});
  let showKey = $state(false);

  async function saveProvider(e: SubmitEvent) {
    e.preventDefault();
    const next = { baseUrl: apiDraft.baseUrl.trim(), model: apiDraft.model.trim(), apiKey: apiDraft.apiKey.trim() };
    apiErrors = {
      baseUrl: !next.baseUrl
        ? "Add the provider's base URL."
        : /^https?:\/\//i.test(next.baseUrl)
          ? undefined
          : "The base URL must start with http:// or https://.",
      model: next.model ? undefined : "Add the model name.",
    };
    if (apiErrors.baseUrl || apiErrors.model) return;
    await save({ plannerApi: next }, `Chat now asks ${next.model}.`);
    if (settings) apiDraft = { ...settings.plannerApi };
  }

  // Bypass needs a second, explicit press before it is saved.
  let modeChoice = $state<PermMode>("ask");
  let confirmBypass = $state(false);

  function pickMode() {
    if (!settings) return;
    if (modeChoice === "bypass" && settings.permissionMode !== "bypass") {
      confirmBypass = true;
      return;
    }
    applyMode(modeChoice);
  }

  async function applyMode(m: PermMode) {
    confirmBypass = false;
    modeChoice = m;
    // A failed save leaves the stored mode in charge, so the choice shows it again.
    if (!(await save({ permissionMode: m }, `New sessions use ${modeLabel(m)}.`))) modeChoice = settings?.permissionMode ?? "ask";
  }

  function keepMode() {
    confirmBypass = false;
    modeChoice = settings?.permissionMode ?? "ask";
  }

  // Applies as soon as it is picked. A failed save puts the choice back to what is stored.
  let sizeChoice = $state(100);

  async function pickSize() {
    await save({ textSize: sizeChoice }, `Text size is ${sizeChoice}%.`);
    sizeChoice = settings?.textSize ?? 100;
  }

  let defaultRoots = $state<string[]>([]);
  let projectCount = $state<number | null>(null);
  let knownFolders = $state<ProjectFolder[]>([]);
  const shownRoots = $derived(settings?.projectRoots.length ? settings.projectRoots : defaultRoots);

  async function countProjects() {
    try {
      knownFolders = await api.projectFolders();
      projectCount = knownFolders.length;
    } catch {
      projectCount = null;
    }
  }

  // Notification rules per CLI and per project folder (PRD FR-41). No rule means all three go out.
  const NOTICES = [
    { key: "waiting", label: "Waiting" },
    { key: "done", label: "Done" },
    { key: "error", label: "Error" },
  ] as const;
  const ALL: NotifyRule = { waiting: true, done: true, error: true };
  const ruleClis = $derived(app.clis.filter((c) => c.path || settings?.notifyClis[c.kind]));
  const ruleFolders = $derived(Object.keys(settings?.notifyProjects ?? {}).sort((a, b) => folderName(a).localeCompare(folderName(b))));
  const addable = $derived(knownFolders.filter((f) => !ruleFolders.some((r) => r.toLowerCase() === f.path.toLowerCase())));
  let folderPick = $state("");

  async function setCliRule(el: HTMLInputElement, kind: CliKind, key: keyof NotifyRule) {
    if (!settings) return;
    const next = { ...(settings.notifyClis[kind] ?? ALL), [key]: el.checked };
    const rules = { ...settings.notifyClis };
    // A rule that sends everything is the same as none.
    if (next.waiting && next.done && next.error) delete rules[kind];
    else rules[kind] = next;
    await saveControl(el, { notifyClis: rules });
  }

  async function setFolderRule(el: HTMLInputElement, folder: string, key: keyof NotifyRule) {
    if (!settings) return;
    await saveControl(el, { notifyProjects: { ...settings.notifyProjects, [folder]: { ...settings.notifyProjects[folder], [key]: el.checked } } });
  }

  async function addFolderRule(folder: string) {
    if (!settings || !folder) return;
    const ok = await save(
      { notifyProjects: { ...settings.notifyProjects, [folder]: { ...ALL } } },
      `Added ${folderName(folder)}. Turn off what it should not send.`,
    );
    if (ok) folderPick = "";
  }

  async function browseFolderRule() {
    const picked = await openDialog({ directory: true, multiple: false, title: "Choose a project folder" });
    if (typeof picked === "string") addFolderRule(picked);
  }

  async function removeFolderRule(folder: string) {
    if (!settings) return;
    const rules = { ...settings.notifyProjects };
    delete rules[folder];
    await save({ notifyProjects: rules }, `${folderName(folder)} sends every notification again.`);
  }

  async function addRoot() {
    if (!settings) return;
    const picked = await openDialog({ directory: true, multiple: false, title: "Choose a folder that holds your projects" });
    if (typeof picked !== "string") return;
    const base = settings.projectRoots.length ? settings.projectRoots : [];
    if (base.some((r) => r.toLowerCase() === picked.toLowerCase())) return;
    await save({ projectRoots: [...base, picked] }, `Added ${picked}.`);
    countProjects();
  }

  async function removeRoot(r: string) {
    if (!settings) return;
    await save({ projectRoots: settings.projectRoots.filter((x) => x !== r) }, `Stopped scanning ${r}.`);
    countProjects();
  }

  async function load() {
    try {
      settings = await loadSettings();
      portDraft = settings.companionPort;
      modeChoice = settings.permissionMode;
      sizeChoice = settings.textSize;
      apiDraft = { ...settings.plannerApi };
      devices = await api.listDevices();
      info = await api.appInfo();
      defaultRoots = await api.defaultProjectRoots();
      countProjects();
      if (on) await newCode();
    } catch (e) {
      loadError = errorText(e);
    }
  }

  onMount(() => {
    load();
    const t = setInterval(() => (now = Date.now()), 1000);
    const d = setInterval(async () => {
      if (on) devices = await api.listDevices().catch(() => devices);
    }, 5000);
    return () => {
      clearInterval(t);
      clearInterval(d);
    };
  });

  /** False when the save failed, so the control that asked can show the stored value again. */
  async function save(patch: Partial<Settings>, message?: string): Promise<boolean> {
    if (!settings) return false;
    try {
      settings = await saveSettings({ ...settings, ...patch });
      if (message) showToast(message);
      return true;
    } catch (e) {
      showToast(errorText(e));
      return false;
    }
  }

  /** Checkboxes and selects show what was picked, not what is stored; a failed save puts them back. */
  async function saveControl(el: HTMLInputElement | HTMLSelectElement, patch: Partial<Settings>, message?: string) {
    const was = el instanceof HTMLInputElement ? !el.checked : null;
    const stored = settings;
    if (await save(patch, message)) return;
    if (el instanceof HTMLInputElement) el.checked = Boolean(was);
    else if (stored) el.value = String(stored[Object.keys(patch)[0] as keyof Settings]);
  }

  async function newCode() {
    pairError = "";
    try {
      pairing = await api.startPairing();
      qrSvg = await QRCode.toString(pairing.url, { type: "svg", margin: 1, color: { dark: "#33361F", light: "#FFFFFF" } });
    } catch (e) {
      pairing = null;
      qrSvg = "";
      pairError = errorText(e);
    }
  }

  async function setPhone(enabled: boolean) {
    if (!settings) return;
    switching = true;
    await save({ companionEnabled: enabled, companionPort: Number(portDraft) || 8765 });
    switching = false;
    if (enabled && app.companion?.running) {
      await newCode();
      showToast(`Phone access is on at ${address}.`);
    } else if (enabled && app.companion?.error) {
      showToast(app.companion.error);
    } else if (!enabled) {
      pairing = null;
      showToast("Phone access is off. Paired phones cannot connect.");
    }
  }

  async function removeDevice(d: Device) {
    try {
      await api.removeDevice(d.id);
      devices = await api.listDevices();
      showToast(`Removed ${d.name}. It has to pair again to connect.`);
    } catch (e) {
      showToast(errorText(e));
    }
  }

  async function clearHistory() {
    if (!clearStep) {
      clearStep = true;
      return;
    }
    try {
      await api.deleteHistory();
      clearStep = false;
      showToast("Finished sessions were deleted with their terminal logs.");
    } catch (e) {
      showToast(errorText(e));
    } finally {
      // Whatever was deleted leaves every list, even when one session could not go.
      refreshSessions();
      info = await api.appInfo().catch(() => info);
    }
  }

  const KEEP = [
    { days: 0, label: "Forever" },
    { days: 90, label: "90 days" },
    { days: 30, label: "30 days" },
    { days: 7, label: "7 days" },
    { days: 1, label: "1 day" },
  ];

  async function pickKeep(el: HTMLSelectElement) {
    const days = Number(el.value);
    await saveControl(el, { keepDays: days }, days ? `Finished sessions older than ${KEEP.find((k) => k.days === days)?.label} are deleted from now on.` : "Finished sessions are kept until you delete them.");
    info = await api.appInfo().catch(() => info);
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

<svelte:head><title>Settings · OpenCompanion</title></svelte:head>

<main class="main" id="settings-main">
  <header class="page-head"><h1 class="grow">Settings</h1></header>

  {#if loadError}
    <div class="state-box" role="alert">
      <h2>Settings could not be loaded</h2>
      <p>{loadError}</p>
      <button class="btn secondary" type="button" onclick={load}>Try again</button>
    </div>
  {:else if !settings}
    <p class="hint" role="status">Loading settings…</p>
  {:else}
    <section class="card" id="phone" aria-labelledby="pa-title">
      <div class="row" style="align-items:flex-start">
        <div class="grow">
          <h2 id="pa-title">Phone access</h2>
          <p class="desc">Lets a phone on the same network watch sessions and answer permission prompts. Off until you turn it on.</p>
        </div>
        <button class="switch" type="button" role="switch" aria-checked={settings.companionEnabled} aria-labelledby="pa-title" disabled={switching} onclick={() => setPhone(!settings?.companionEnabled)}></button>
      </div>

      {#if !settings.companionEnabled}
        <div class="off-note">
          <p class="meta" style="margin:0;font-size:14px">Phone access is off. Nothing is listening on your network.</p>
          <div class="row">
            <label class="port">
              <span class="meta">Port</span>
              <input class="input mono" type="number" min="1024" max="65535" bind:value={portDraft} />
            </label>
            <button class="btn primary" type="button" disabled={switching} onclick={() => setPhone(true)}><DeviceMobile size={16} aria-hidden="true" />Turn on phone access</button>
          </div>
        </div>
      {:else if app.companion?.error}
        <div class="warn" role="alert"><Warning size={18} aria-hidden="true" /><span>{app.companion.error} Pick another port, then turn phone access off and on again.</span></div>
        <label class="port"><span class="meta">Port</span><input class="input mono" type="number" min="1024" max="65535" bind:value={portDraft} /></label>
      {:else}
        <div class="pair">
          <div class="qr-col">
            {#if qrSvg && left > 0}
              <div class="qr" role="img" aria-label="Pairing QR code for {pairing?.url}">{@html qrSvg}</div>
              <span class="meta">Pairing code</span>
              <span class="code">{pairing?.code.slice(0, 3)} {pairing?.code.slice(3)}</span>
              <!-- Not a live region: a screen reader would read the countdown every second. The expiry is announced once, below. -->
              <span class="meta">Expires in {Math.floor(left / 60)}:{String(left % 60).padStart(2, "0")}</span>
            {:else}
              <div class="qr empty"><span class="meta" role="status">{pairError || "The code expired. Press New code for another."}</span></div>
            {/if}
            <button class="btn secondary sm" type="button" style="align-self:flex-start" onclick={newCode}><ArrowClockwise size={16} aria-hidden="true" />New code</button>
          </div>
          <div class="pair-info">
            <div class="field">
              <span class="label">Address on this network</span>
              <div class="addr">
                <code>{address || "No network address found"}</code>
                {#if address}<button class="icon-btn" type="button" aria-label="Copy address" onclick={() => copy(address)}><Copy size={18} aria-hidden="true" /></button>{/if}
              </div>
            </div>
            <p style="margin:0;font-size:14px;line-height:1.45">On your phone, join the same Wi-Fi and scan the code with the camera. The phone asks for a name, then shows your sessions. You can also open the address and type the code.</p>
            <div class="warn"><Warning size={18} aria-hidden="true" /><span>This connection uses plain HTTP. On a network you do not trust, or away from home, reach your computer through a VPN with HTTPS, such as Tailscale.</span></div>
          </div>
        </div>
        <div class="field">
          <span class="label">Paired devices</span>
          {#if devices.length === 0}
            <p class="meta" style="margin:0">No phones paired yet.</p>
          {:else}
            <div class="devices">
              {#each devices as d (d.id)}
                <div class="device">
                  <DeviceMobile size={20} aria-hidden="true" />
                  <span class="grow"><b>{d.name}</b><span class="meta">{d.lastSeen ? `Last seen ${ago(d.lastSeen, now)}` : "Not seen yet"}</span></span>
                  <button class="btn secondary sm" type="button" onclick={() => removeDevice(d)}>Remove</button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      {#if !on && settings.companionEnabled && !app.companion?.error}
        <p class="meta" role="status" style="margin:0">Starting…</p>
      {/if}
    </section>

    <section class="card" id="permissions" aria-labelledby="perm-title">
      <div>
        <h2 id="perm-title">Permission mode</h2>
        <p class="desc">How much the CLIs may do on their own in sessions OpenCompanion starts. New session and Board runs can pick another mode for a single session. Chat's planner is not affected: it stays read-only.</p>
      </div>
      <fieldset class="bare">
        <legend class="sr-only">Default permission mode</legend>
        <div class="modes">
          {#each MODES as m (m.id)}
            <label class="opt stack" class:bypass={m.id === "bypass"}>
              <input type="radio" name="perm-mode" value={m.id} bind:group={modeChoice} onchange={pickMode} />
              <b>{m.label}</b>
              <p>{m.short}</p>
            </label>
          {/each}
        </div>
      </fieldset>
      {#if confirmBypass}
        <div class="warn bypass-warn" role="alert">
          <Warning size={18} aria-hidden="true" />
          <div class="grow">
            <p style="margin:0 0 10px"><b>Turn on Bypass for every new session?</b> The CLIs will change or delete files and run any command without asking, and nothing will reach Needs you. Sessions that are already running keep their mode.</p>
            <div class="row" style="gap:10px;flex-wrap:wrap">
              <button class="btn danger" type="button" onclick={() => applyMode("bypass")}>Use Bypass</button>
              <button class="btn secondary" type="button" onclick={keepMode}>Keep {modeLabel(settings.permissionMode)}</button>
            </div>
          </div>
        </div>
      {:else}
        <p class="meta" style="margin:0">{MODES.find((m) => m.id === settings?.permissionMode)?.detail}</p>
      {/if}
      <div class="table-wrap">
        <table>
          <caption class="sr-only">What each mode passes to each CLI</caption>
          <thead><tr><th scope="col">Mode</th><th scope="col">Claude Code</th><th scope="col">Codex CLI</th><th scope="col">OpenCode</th></tr></thead>
          <tbody>
            {#each MODES as m (m.id)}
              <tr class:current={settings.permissionMode === m.id}>
                <th scope="row">{m.label}</th>
                <td class="mono">{m.flags.claude}</td>
                <td class="mono">{m.flags.codex}</td>
                <td class="mono">{m.flags.opencode}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

    <div class="two">
      <section class="card" id="chat" aria-labelledby="planner-title">
        <h2 id="planner-title">Chat planner</h2>
        <label class="meta" for="planner-select">What turns your chat messages into session suggestions: a CLI running headless, or a model you reach through an OpenAI-compatible API. Neither can run commands or change files.</label>
        <select
          class="select"
          id="planner-select"
          value={usingApi ? "api" : (settings.chatCli ?? planners[0]?.kind ?? "")}
          onchange={(e) => pickPlanner(e.currentTarget)}
        >
          {#each planners as c (c.kind)}<option value={c.kind}>{c.label} {c.version ?? ""}</option>{/each}
          {#if planners.length === 0}<option value="">No CLI can plan yet</option>{/if}
          <option value="api">Custom provider (OpenAI-compatible API)</option>
        </select>
        {#if usingApi}
          <form class="provider" id="provider-form" novalidate onsubmit={saveProvider}>
            <div class="field">
              <label class="label" for="provider-url">Base URL</label>
              <input
                class="input mono"
                id="provider-url"
                bind:value={apiDraft.baseUrl}
                oninput={() => (apiErrors.baseUrl = undefined)}
                placeholder="Starts with http:// or https://"
                spellcheck="false"
                autocomplete="off"
                aria-invalid={apiErrors.baseUrl ? "true" : undefined}
                aria-describedby="provider-url-help{apiErrors.baseUrl ? ' provider-url-error' : ''}"
              />
              <p class="help" id="provider-url-help">The address before /chat/completions. OpenRouter: https://openrouter.ai/api/v1. Ollama: http://localhost:11434/v1. LM Studio: http://localhost:1234/v1.</p>
              {#if apiErrors.baseUrl}<p class="error" id="provider-url-error">{apiErrors.baseUrl}</p>{/if}
            </div>
            <div class="field">
              <label class="label" for="provider-model">Model</label>
              <input
                class="input mono"
                id="provider-model"
                bind:value={apiDraft.model}
                oninput={() => (apiErrors.model = undefined)}
                placeholder="As the provider writes it"
                spellcheck="false"
                autocomplete="off"
                aria-invalid={apiErrors.model ? "true" : undefined}
                aria-describedby={apiErrors.model ? "provider-model-error" : undefined}
              />
              {#if apiErrors.model}<p class="error" id="provider-model-error">{apiErrors.model}</p>{/if}
            </div>
            <div class="field">
              <label class="label" for="provider-key">API key</label>
              <div class="row" style="gap:10px">
                <input
                  class="input mono grow"
                  id="provider-key"
                  type={showKey ? "text" : "password"}
                  bind:value={apiDraft.apiKey}
                  placeholder="Empty for Ollama or LM Studio"
                  spellcheck="false"
                  autocomplete="off"
                  aria-describedby="provider-key-help"
                />
                <button class="btn secondary" type="button" aria-pressed={showKey} onclick={() => (showKey = !showKey)}>{showKey ? "Hide" : "Show"}</button>
              </div>
              <p class="help" id="provider-key-help">Kept in OpenCompanion's database on this computer and sent only to the base URL above.</p>
            </div>
            <button class="btn primary" type="submit" id="btn-save-provider">Save provider</button>
          </form>
        {/if}
        <label class="check-row">
          <input type="checkbox" checked={settings.plannerCanRead && !usingApi} disabled={usingApi} onchange={(e) => saveControl(e.currentTarget, { plannerCanRead: e.currentTarget.checked }, "Planner access saved.")} />
          <span>Let the planner read my project folders</span>
        </label>
        <p class="meta" style="margin:0">
          {#if usingApi}A custom provider has no file tools, so it sees folder names and what they hold (git, package.json), not the files. Pick a CLI to let the planner read them.
          {:else}Read-only: it can look at files to find the right project and write a sharper prompt. It cannot run commands or change anything. Off means it only sees folder names.{/if}
        </p>
      </section>

      <section class="card" id="projects" aria-labelledby="projects-title">
        <h2 id="projects-title">Project folders</h2>
        <p class="meta" style="margin:0">
          Where your projects live. OpenCompanion lists the folders inside them, plus folders you used before, for Chat and New session.
          {#if projectCount !== null}{projectCount} projects found.{/if}
        </p>
        <ul class="roots">
          {#each shownRoots as r (r)}
            <li>
              <span class="mono grow" title={r}>{r}</span>
              {#if settings.projectRoots.length}
                <button class="btn ghost sm" type="button" aria-label="Stop scanning {r}" onclick={() => removeRoot(r)}>Remove</button>
              {/if}
            </li>
          {/each}
          {#if shownRoots.length === 0}<li class="meta">No project folder found yet. Add the folder that holds your projects.</li>{/if}
        </ul>
        {#if !settings.projectRoots.length && shownRoots.length}<p class="meta" style="margin:0">Found automatically. Adding a folder replaces this list.</p>{/if}
        <button class="btn secondary" type="button" style="align-self:flex-start" onclick={addRoot}>Add folder</button>
      </section>

      <section class="card" id="notifications" aria-labelledby="notif-title">
        <h2 id="notif-title">Notifications</h2>
        <div class="checks">
          <label><input type="checkbox" checked={settings.notifyWaiting} onchange={(e) => saveControl(e.currentTarget, { notifyWaiting: e.currentTarget.checked })} />A session is waiting for you</label>
          <label><input type="checkbox" checked={settings.notifyDone} onchange={(e) => saveControl(e.currentTarget, { notifyDone: e.currentTarget.checked })} />A session is done</label>
          <label><input type="checkbox" checked={settings.notifyError} onchange={(e) => saveControl(e.currentTarget, { notifyError: e.currentTarget.checked })} />A session stopped with an error</label>
        </div>
        <table class="rules" id="notify-by-cli">
          <caption>By CLI</caption>
          <thead><tr><th scope="col"><span class="sr-only">CLI</span></th>{#each NOTICES as n (n.key)}<th scope="col">{n.label}</th>{/each}</tr></thead>
          <tbody>
            {#each ruleClis as c (c.kind)}
              {@const rule = settings.notifyClis[c.kind] ?? ALL}
              <tr>
                <th scope="row">{c.label}</th>
                {#each NOTICES as n (n.key)}
                  <td><input type="checkbox" aria-label="{c.label}: {n.label}" checked={rule[n.key]} onchange={(e) => setCliRule(e.currentTarget, c.kind, n.key)} /></td>
                {/each}
              </tr>
            {:else}
              <tr><td colspan="4" class="meta">No CLI is installed yet.</td></tr>
            {/each}
          </tbody>
        </table>
        <table class="rules" id="notify-by-folder">
          <caption>By project folder</caption>
          {#if ruleFolders.length}
            <thead><tr><th scope="col"><span class="sr-only">Folder</span></th>{#each NOTICES as n (n.key)}<th scope="col">{n.label}</th>{/each}<th scope="col"><span class="sr-only">Remove</span></th></tr></thead>
          {/if}
          <tbody>
            {#each ruleFolders as f (f)}
              {@const rule = settings.notifyProjects[f]}
              <tr>
                <th scope="row"><span class="mono" title={f}>{folderName(f)}</span></th>
                {#each NOTICES as n (n.key)}
                  <td><input type="checkbox" aria-label="{folderName(f)}: {n.label}" checked={rule[n.key]} onchange={(e) => setFolderRule(e.currentTarget, f, n.key)} /></td>
                {/each}
                <td><button class="btn ghost sm" type="button" aria-label="Remove the rule for {folderName(f)}" onclick={() => removeFolderRule(f)}>Remove</button></td>
              </tr>
            {:else}
              <tr><td colspan="5" class="meta">Every project folder sends all of them.</td></tr>
            {/each}
          </tbody>
        </table>
        <div class="add-rule">
          <label class="sr-only" for="notify-folder">Project folder to add a rule for</label>
          <select class="select" id="notify-folder" bind:value={folderPick}>
            <option value="">Choose a project folder…</option>
            {#each addable as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
          </select>
          <button class="btn secondary sm" type="button" disabled={!folderPick} onclick={() => addFolderRule(folderPick)}>Add</button>
          <button class="btn ghost sm" type="button" onclick={browseFolderRule}>Browse…</button>
        </div>
        <p class="meta" style="margin:0">A notification goes out only when the switch above, its CLI and its folder all allow it. The phone follows the same rules.</p>
      </section>

      <section class="card" id="text-size" aria-labelledby="text-title">
        <h2 id="text-title">Text size</h2>
        <p class="meta" style="margin:0">Makes text, and the buttons and spacing around it, bigger or smaller on every screen. The phone page keeps the text size set on the phone.</p>
        <fieldset class="bare">
          <legend class="sr-only">Text size</legend>
          <div class="sizes">
            {#each TEXT_SIZES as s (s)}
              <label class="opt stack">
                <input type="radio" name="text-size" value={s} bind:group={sizeChoice} onchange={pickSize} />
                <span class="sample" style="font-size:{(15 * s) / 100}px" aria-hidden="true">A</span>
                <b>{s}%</b>
                {#if s === 100}<small>Default</small>{/if}
              </label>
            {/each}
          </div>
        </fieldset>
      </section>

      <section class="card" id="closing" aria-labelledby="close-title">
        <h2 id="close-title">Window and sign-in</h2>
        <label class="check-row">
          <input
            type="checkbox"
            checked={settings.closeToTray}
            onchange={(e) =>
              saveControl(e.currentTarget, { closeToTray: e.currentTarget.checked }, e.currentTarget.checked ? "Closing the window now keeps OpenCompanion in the tray." : "Closing the window now quits OpenCompanion.")}
          />
          <span>Keep running in the tray</span>
        </label>
        <p class="meta" style="margin:0">
          {settings.closeToTray
            ? "Sessions keep running and your phone can still reach them. Click the tray icon to open the window again; quit from its menu to stop the sessions."
            : "Closing the window quits OpenCompanion and stops every session it started."}
        </p>
        {#if info?.canStartAtLogin}
          <label class="check-row">
            <input
              type="checkbox"
              checked={settings.startAtLogin}
              onchange={(e) =>
                saveControl(
                  e.currentTarget,
                  { startAtLogin: e.currentTarget.checked },
                  e.currentTarget.checked ? "OpenCompanion starts in the tray when you sign in." : "OpenCompanion no longer starts when you sign in.",
                )}
            />
            <span>Start in the tray when I sign in to Windows</span>
          </label>
        {/if}
      </section>

      <section class="card" aria-labelledby="scan-title">
        <h2 id="scan-title">Outside sessions</h2>
        <label class="meta" for="scan-select">How often OpenCompanion looks for CLIs running in other terminals. The scan reads the process list only.</label>
        <select class="select" id="scan-select" value={String(settings.scanSeconds)} onchange={(e) => saveControl(e.currentTarget, { scanSeconds: Number(e.currentTarget.value) }, "Scan interval saved.")}>
          <option value="5">Every 5 seconds</option>
          <option value="10">Every 10 seconds</option>
          <option value="30">Every 30 seconds</option>
          <option value="60">Every minute</option>
        </select>
      </section>

      <section class="card" aria-labelledby="history-title">
        <h2 id="history-title">History</h2>
        <p class="meta" style="margin:0">
          Sessions, their events and terminal logs are stored on this computer only{info ? `, in ${info.dataDir}` : ""}.
          {#if info}{info.counts.sessions} sessions are stored.{/if}
        </p>
        <label class="meta" for="keep-select">Keep finished sessions for</label>
        <select class="select" id="keep-select" value={String(settings.keepDays)} onchange={(e) => pickKeep(e.currentTarget)}>
          {#each KEEP as k (k.days)}<option value={String(k.days)}>{k.label}</option>{/each}
        </select>
        <p class="meta" style="margin:0">Older ones go with their events and terminal logs, checked every hour. Files the CLIs changed in your projects are never touched.</p>
        <button class="btn secondary" type="button" style="align-self:flex-start" onclick={clearHistory}>
          {clearStep ? "Press again to delete finished sessions" : "Delete finished sessions"}
        </button>
      </section>
    </div>
    {#if info}<p class="meta note" style="margin:0">OpenCompanion {info.version}</p>{/if}
  {/if}
</main>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .card h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 800;
  }
  .desc {
    margin: 4px 0 0;
    font-size: 14px;
    color: var(--ink-2);
    max-width: 640px;
  }
  .pair {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 28px;
  }
  .qr-col,
  .pair-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .pair-info {
    gap: 16px;
  }
  .qr {
    width: 180px;
    height: 180px;
    border-radius: var(--r-md);
    background: #ffffff;
    border: 1px solid var(--line);
    display: grid;
    place-items: center;
    padding: 6px;
  }
  .qr :global(svg) {
    width: 100%;
    height: 100%;
  }
  .qr.empty {
    background: var(--bg);
    padding: 16px;
    text-align: center;
  }
  .code {
    font: 600 20px var(--font-mono);
    letter-spacing: 1px;
  }
  .addr {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .addr code {
    padding: 8px 12px;
    border-radius: var(--r-sm);
    background: var(--bg);
    border: 1px solid var(--line);
    font: 13px var(--font-mono);
    overflow-wrap: anywhere;
  }
  .warn {
    display: flex;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--r-btn);
    background: var(--surface-2);
    font-size: 13px;
    line-height: 1.45;
  }
  .devices {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .device {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-btn);
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .device :global(svg) {
    color: var(--ink-2);
  }
  .device b {
    display: block;
    font-size: 14px;
  }
  .off-note {
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }
  .port {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .port .input {
    width: 110px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 20px;
  }
  .two .card {
    gap: 12px;
    padding: 20px;
  }
  .two .card h2 {
    font-size: 16px;
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .checks label {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
    cursor: pointer;
  }
  .checks input,
  .rules input {
    width: 18px;
    height: 18px;
    accent-color: var(--forest);
  }
  .rules {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    min-width: 0;
  }
  .rules caption {
    text-align: left;
    font-weight: 800;
    font-size: 14px;
    padding-bottom: 6px;
  }
  .rules th,
  .rules td {
    padding: 6px 8px;
    border-top: 1px solid var(--line);
  }
  .rules thead th {
    border-top: 0;
    background: none;
    color: var(--ink-2);
    font-weight: 700;
    text-align: center;
  }
  .rules tbody th {
    text-align: left;
    font-weight: 700;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rules td {
    text-align: center;
  }
  .rules td.meta {
    text-align: left;
  }
  .add-rule {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .add-rule .select {
    flex: 1;
    min-width: 180px;
  }
  .select {
    appearance: auto;
  }
  .bare {
    border: 0;
    padding: 0;
    margin: 0;
  }
  .modes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .sizes {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 8px;
  }
  .sizes .opt {
    align-items: center;
    padding: 8px 4px;
  }
  .sizes .opt:has(input:checked) {
    padding: 7px 3px;
  }
  /* Same box height for every sample, so the letters grow from one baseline. */
  .sample {
    display: flex;
    align-items: flex-end;
    height: 26px;
    line-height: 1;
    font-weight: 800;
  }
  .modes .opt.bypass:has(input:checked) {
    border-color: var(--st-err);
  }
  .bypass-warn {
    border: 1.5px solid var(--st-err);
    background: var(--surface);
  }
  .bypass-warn > :global(svg) {
    color: var(--st-err);
  }
  .table-wrap {
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    min-width: 640px;
  }
  th,
  td {
    text-align: left;
    padding: 9px 12px;
    border-top: 1px solid var(--line);
  }
  thead th {
    border-top: 0;
    background: var(--surface-2);
    color: var(--ink-2);
    font-weight: 700;
  }
  tbody th {
    font-weight: 700;
    white-space: nowrap;
  }
  td.mono {
    font-size: 12px;
  }
  tr.current {
    background: var(--surface-2);
  }
  .check-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 32px;
    cursor: pointer;
    font-weight: 700;
  }
  .check-row input {
    width: 18px;
    height: 18px;
    accent-color: var(--forest);
  }
  .check-row:has(input:disabled) {
    cursor: not-allowed;
    color: var(--ink-2);
  }
  .provider {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .provider .help {
    overflow-wrap: anywhere;
  }
  .provider > .btn {
    align-self: flex-start;
  }
  .roots {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .roots li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--r-btn);
    border: 1px solid var(--line);
    background: var(--bg);
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 1100px) {
    .modes {
      grid-template-columns: 1fr 1fr;
    }
    .pair {
      grid-template-columns: 1fr;
    }
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
