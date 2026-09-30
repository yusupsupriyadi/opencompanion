<script lang="ts">
  import { page } from "$app/state";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import Copy from "phosphor-svelte/lib/Copy";
  import DeviceMobile from "phosphor-svelte/lib/DeviceMobile";
  import Moon from "phosphor-svelte/lib/Moon";
  import Sun from "phosphor-svelte/lib/Sun";
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
  import { forgetSaved } from "$lib/command-history";
  import { MODES, TEXT_SIZES, ago, folderName, modeLabel, shortPath } from "$lib/format";
  import { LANGS, plural, t, tb, type Key } from "$lib/i18n.svelte";
  import SettingsHead from "$lib/SettingsHead.svelte";
  import { sectionOf } from "$lib/settings-nav";
  import { app, currentTheme, loadSettings, refreshSessions, saveSettings, setTheme, showToast } from "$lib/store.svelte";

  // One section at a time, the one picked in the Settings sidebar. Its name is the page's H1.
  const section = $derived(sectionOf(page.url));

  let settings = $state<Settings | null>(null);
  let loadError = $state("");
  let pairing = $state<Pairing | null>(null);
  let qrSvg = $state("");
  let pairError = $state("");
  let devices = $state<Device[]>([]);
  let info = $state<AppInfo | null>(null);
  let switching = $state(false);
  let clearStep = $state(false);
  let commandsStep = $state(false);
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
      ok = await save({ plannerSource: "api" }, done ? t("settings.planner.saved") : t("settings.planner.savedNeedsProvider"));
    } else {
      ok = await save({ plannerSource: "cli", chatCli: value as Settings["chatCli"] }, t("settings.planner.saved"));
    }
    if (!ok) el.value = usingApi ? "api" : (settings?.chatCli ?? planners[0]?.kind ?? "");
  }

  // The provider fields are saved together, on Save provider, so a half-typed URL is never used.
  let apiDraft = $state<PlannerApi>({ baseUrl: "", model: "", apiKey: "" });
  // Keys, not text, so an error on screen follows a language switch.
  let apiErrors = $state<{ baseUrl?: Key; model?: Key }>({});
  let showKey = $state(false);

  async function saveProvider(e: SubmitEvent) {
    e.preventDefault();
    const next = { baseUrl: apiDraft.baseUrl.trim(), model: apiDraft.model.trim(), apiKey: apiDraft.apiKey.trim() };
    apiErrors = {
      baseUrl: !next.baseUrl
        ? "settings.provider.needUrl"
        : /^https?:\/\//i.test(next.baseUrl)
          ? undefined
          : "settings.provider.badUrl",
      model: next.model ? undefined : "settings.provider.needModel",
    };
    if (apiErrors.baseUrl || apiErrors.model) return;
    await save({ plannerApi: next }, t("settings.provider.saved", { model: next.model }));
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
    if (!(await save({ permissionMode: m }, t("settings.perm.saved", { mode: modeLabel(m) })))) modeChoice = settings?.permissionMode ?? "ask";
  }

  function keepMode() {
    confirmBypass = false;
    modeChoice = settings?.permissionMode ?? "ask";
  }

  // Applies as soon as it is picked. A failed save puts the choice back to what is stored.
  let sizeChoice = $state(100);

  async function pickSize() {
    await save({ textSize: sizeChoice }, t("settings.text.saved", { size: sizeChoice }));
    sizeChoice = settings?.textSize ?? 100;
  }

  async function pickLanguage(el: HTMLSelectElement) {
    // The toast is written after the save, so it reads in the language just picked.
    if (await saveControl(el, { language: el.value as Settings["language"] })) showToast(t("settings.language.saved"));
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
    { key: "waiting", label: "settings.notify.col.waiting" },
    { key: "done", label: "settings.notify.col.done" },
    { key: "error", label: "settings.notify.col.error" },
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
      t("settings.notify.added", { folder: folderName(folder) }),
    );
    if (ok) folderPick = "";
  }

  async function browseFolderRule() {
    const picked = await openDialog({ directory: true, multiple: false, title: t("settings.notify.dialogTitle") });
    if (typeof picked === "string") addFolderRule(picked);
  }

  // Folders whose cards start without Run (PRD FR-26). Adding one takes a second, explicit press.
  let autoPick = $state("");
  let autoConfirm = $state("");
  const autoAddable = $derived(knownFolders.filter((f) => !settings?.autoRunFolders.some((a) => a.toLowerCase() === f.path.toLowerCase())));

  async function allowAutoRun(folder: string) {
    if (!settings) return;
    const ok = await save({ autoRunFolders: [...settings.autoRunFolders, folder] }, t("settings.autoRun.added", { folder: folderName(folder) }));
    if (ok) {
      autoConfirm = "";
      autoPick = "";
    }
  }

  async function browseAutoRun() {
    const picked = await openDialog({ directory: true, multiple: false, title: t("settings.autoRun.dialogTitle") });
    if (typeof picked === "string") autoConfirm = picked;
  }

  async function removeAutoRun(folder: string) {
    if (!settings) return;
    await save({ autoRunFolders: settings.autoRunFolders.filter((f) => f !== folder) }, t("settings.autoRun.removed", { folder: folderName(folder) }));
  }

  async function removeFolderRule(folder: string) {
    if (!settings) return;
    const rules = { ...settings.notifyProjects };
    delete rules[folder];
    await save({ notifyProjects: rules }, t("settings.notify.removed", { folder: folderName(folder) }));
  }

  async function addRoot() {
    if (!settings) return;
    const picked = await openDialog({ directory: true, multiple: false, title: t("settings.projects.dialogTitle") });
    if (typeof picked !== "string") return;
    const base = settings.projectRoots.length ? settings.projectRoots : [];
    if (base.some((r) => r.toLowerCase() === picked.toLowerCase())) return;
    await save({ projectRoots: [...base, picked] }, t("settings.projects.added", { path: picked }));
    countProjects();
  }

  async function removeRoot(r: string) {
    if (!settings) return;
    await save({ projectRoots: settings.projectRoots.filter((x) => x !== r) }, t("settings.projects.removed", { path: r }));
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
      showToast(tb(errorText(e)));
      return false;
    }
  }

  /** Checkboxes and selects show what was picked, not what is stored; a failed save puts them back. */
  async function saveControl(el: HTMLInputElement | HTMLSelectElement, patch: Partial<Settings>, message?: string): Promise<boolean> {
    const was = el instanceof HTMLInputElement ? !el.checked : null;
    const stored = settings;
    if (await save(patch, message)) return true;
    if (el instanceof HTMLInputElement) el.checked = Boolean(was);
    else if (stored) el.value = String(stored[Object.keys(patch)[0] as keyof Settings]);
    return false;
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
      showToast(t("settings.phone.onToast", { address }));
    } else if (enabled && app.companion?.error) {
      showToast(app.companion.error);
    } else if (!enabled) {
      pairing = null;
      showToast(t("settings.phone.offToast"));
    }
  }

  async function removeDevice(d: Device) {
    try {
      await api.removeDevice(d.id);
      devices = await api.listDevices();
      showToast(t("settings.phone.removed", { name: d.name }));
    } catch (e) {
      showToast(tb(errorText(e)));
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
      showToast(t("settings.history.deleted"));
    } catch (e) {
      showToast(tb(errorText(e)));
    } finally {
      // Whatever was deleted leaves every list, even when one session could not go.
      refreshSessions();
      info = await api.appInfo().catch(() => info);
    }
  }

  async function deleteCommands() {
    if (!commandsStep) {
      commandsStep = true;
      return;
    }
    try {
      const n = await api.deleteShellHistory();
      forgetSaved();
      commandsStep = false;
      showToast(plural(n, "settings.commands.deletedOne", "settings.commands.deleted"));
    } catch (e) {
      showToast(tb(errorText(e)));
    }
  }

  // Days a finished session is kept; 0 keeps it until it is deleted by hand.
  const KEEP = [0, 90, 30, 7, 1];

  function keepLabel(days: number) {
    return days ? plural(days, "settings.history.dayOne", "settings.history.days") : t("settings.history.forever");
  }

  async function pickKeep(el: HTMLSelectElement) {
    const days = Number(el.value);
    await saveControl(el, { keepDays: days }, days ? t("settings.history.keepSaved", { age: keepLabel(days) }) : t("settings.history.keepForever"));
    info = await api.appInfo().catch(() => info);
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      showToast(t("settings.copied", { text }));
    } catch {
      showToast(t("settings.copyFailed"));
    }
  }
</script>

<svelte:head><title>{t("settings.pageTitle")}</title></svelte:head>

<main class="main settings-main" id="settings-main">
  <SettingsHead />

  {#if loadError}
    <div class="state-box" role="alert">
      <h2>{t("settings.loadError")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={load}>{t("settings.tryAgain")}</button>
    </div>
  {:else if !settings}
    <p class="hint" role="status">{t("settings.loading")}</p>
  {:else}
    {#if section === "phone"}
      <section class="card" id="phone" aria-labelledby="settings-title">
        <div class="row" style="align-items:flex-start">
          <p class="desc grow">{t("settings.phone.desc")}</p>
          <button class="switch" type="button" role="switch" aria-checked={settings.companionEnabled} aria-labelledby="settings-title" disabled={switching} onclick={() => setPhone(!settings?.companionEnabled)}></button>
        </div>

        {#if !settings.companionEnabled}
          <div class="off-note">
            <p class="meta" style="margin:0;font-size:14px">{t("settings.phone.offNote")}</p>
            <div class="row">
              <label class="port">
                <span class="meta">{t("settings.phone.port")}</span>
                <input class="input mono" type="number" min="1024" max="65535" bind:value={portDraft} />
              </label>
              <button class="btn primary" type="button" disabled={switching} onclick={() => setPhone(true)}><DeviceMobile size={16} aria-hidden="true" />{t("settings.phone.turnOn")}</button>
            </div>
          </div>
        {:else if app.companion?.error}
          <div class="warn" role="alert"><Warning size={18} aria-hidden="true" /><span>{t("settings.phone.portError", { error: app.companion.error })}</span></div>
          <label class="port"><span class="meta">{t("settings.phone.port")}</span><input class="input mono" type="number" min="1024" max="65535" bind:value={portDraft} /></label>
        {:else}
          <div class="pair">
            <div class="qr-col">
              {#if qrSvg && left > 0}
                <div class="qr" role="img" aria-label={t("settings.phone.qrLabel", { url: pairing?.url ?? "" })}>{@html qrSvg}</div>
                <span class="meta">{t("settings.phone.code")}</span>
                <span class="code">{pairing?.code.slice(0, 3)} {pairing?.code.slice(3)}</span>
                <!-- Not a live region: a screen reader would read the countdown every second. The expiry is announced once, below. -->
                <span class="meta">{t("settings.phone.expiresIn", { time: `${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}` })}</span>
              {:else}
                <div class="qr empty"><span class="meta" role="status">{pairError || t("settings.phone.expired")}</span></div>
              {/if}
              <button class="btn secondary sm" type="button" style="align-self:flex-start" onclick={newCode}><ArrowClockwise size={16} aria-hidden="true" />{t("settings.phone.newCode")}</button>
            </div>
            <div class="pair-info">
              <div class="field">
                <span class="label">{t("settings.phone.address")}</span>
                <div class="addr">
                  <code>{address || t("settings.phone.noAddress")}</code>
                  {#if address}<button class="icon-btn" type="button" aria-label={t("settings.phone.copyAddress")} onclick={() => copy(address)}><Copy size={18} aria-hidden="true" /></button>{/if}
                </div>
              </div>
              <p style="margin:0;font-size:14px;line-height:1.45">{t("settings.phone.howTo")}</p>
              <div class="warn"><Warning size={18} aria-hidden="true" /><span>{t("settings.phone.http")}</span></div>
            </div>
          </div>
          <div class="field">
            <span class="label">{t("settings.phone.devices")}</span>
            {#if devices.length === 0}
              <p class="meta" style="margin:0">{t("settings.phone.noDevices")}</p>
            {:else}
              <div class="devices">
                {#each devices as d (d.id)}
                  <div class="device">
                    <DeviceMobile size={20} aria-hidden="true" />
                    <span class="grow"><b>{d.name}</b><span class="meta">{d.lastSeen ? t("settings.phone.lastSeen", { when: ago(d.lastSeen, now) }) : t("settings.phone.notSeen")}</span></span>
                    <button class="btn secondary sm" type="button" onclick={() => removeDevice(d)}>{t("settings.remove")}</button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
        {#if !on && settings.companionEnabled && !app.companion?.error}
          <p class="meta" role="status" style="margin:0">{t("settings.phone.starting")}</p>
        {/if}
      </section>
    {:else if section === "permissions"}
      <section class="card" id="permissions" aria-labelledby="settings-title">
        <p class="desc">{t("settings.perm.desc")}</p>
        <fieldset class="bare">
          <legend class="sr-only">{t("settings.perm.legend")}</legend>
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
              <p style="margin:0 0 10px"><b>{t("settings.perm.bypassAsk")}</b> {t("settings.perm.bypassWhat")}</p>
              <div class="row" style="gap:10px;flex-wrap:wrap">
                <button class="btn danger" type="button" onclick={() => applyMode("bypass")}>{t("settings.perm.useBypass")}</button>
                <button class="btn secondary" type="button" onclick={keepMode}>{t("settings.perm.keep", { mode: modeLabel(settings.permissionMode) })}</button>
              </div>
            </div>
          </div>
        {:else}
          <p class="meta" style="margin:0">{MODES.find((m) => m.id === settings?.permissionMode)?.detail}</p>
        {/if}
        <div class="table-wrap">
          <table>
            <caption class="sr-only">{t("settings.perm.caption")}</caption>
            <thead><tr><th scope="col">{t("settings.perm.mode")}</th><th scope="col">Claude Code</th><th scope="col">Codex CLI</th><th scope="col">OpenCode</th><th scope="col">Pi</th><th scope="col">omp</th><th scope="col">Cursor CLI</th></tr></thead>
            <tbody>
              {#each MODES as m (m.id)}
                <tr class:current={settings.permissionMode === m.id}>
                  <th scope="row">{m.label}</th>
                  <td class="mono">{m.flags.claude}</td>
                  <td class="mono">{m.flags.codex}</td>
                  <td class="mono">{m.flags.opencode}</td>
                  <td class="mono">{m.flags.pi}</td>
                  <td class="mono">{m.flags.omp}</td>
                  <td class="mono">{m.flags.cursor}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </section>
    {:else if section === "planner"}
      <section class="card" id="chat" aria-labelledby="settings-title">
        <div class="field">
          <label class="meta" for="planner-select">{t("settings.planner.label")}</label>
          <select
            class="select"
            id="planner-select"
            value={usingApi ? "api" : (settings.chatCli ?? planners[0]?.kind ?? "")}
            onchange={(e) => pickPlanner(e.currentTarget)}
          >
            {#each planners as c (c.kind)}<option value={c.kind}>{c.label} {c.version ?? ""}</option>{/each}
            {#if planners.length === 0}<option value="">{t("settings.planner.noCli")}</option>{/if}
            <option value="api">{t("settings.planner.custom")}</option>
          </select>
        </div>
        {#if usingApi}
          <form class="provider" id="provider-form" novalidate onsubmit={saveProvider}>
            <div class="field">
              <label class="label" for="provider-url">{t("settings.provider.baseUrl")}</label>
              <input
                class="input mono"
                id="provider-url"
                bind:value={apiDraft.baseUrl}
                oninput={() => (apiErrors.baseUrl = undefined)}
                placeholder={t("settings.provider.baseUrlPlaceholder")}
                spellcheck="false"
                autocomplete="off"
                aria-invalid={apiErrors.baseUrl ? "true" : undefined}
                aria-describedby="provider-url-help{apiErrors.baseUrl ? ' provider-url-error' : ''}"
              />
              <p class="help" id="provider-url-help">{t("settings.provider.baseUrlHelp")}</p>
              {#if apiErrors.baseUrl}<p class="error" id="provider-url-error">{t(apiErrors.baseUrl)}</p>{/if}
            </div>
            <div class="field">
              <label class="label" for="provider-model">{t("settings.provider.model")}</label>
              <input
                class="input mono"
                id="provider-model"
                bind:value={apiDraft.model}
                oninput={() => (apiErrors.model = undefined)}
                placeholder={t("settings.provider.modelPlaceholder")}
                spellcheck="false"
                autocomplete="off"
                aria-invalid={apiErrors.model ? "true" : undefined}
                aria-describedby={apiErrors.model ? "provider-model-error" : undefined}
              />
              {#if apiErrors.model}<p class="error" id="provider-model-error">{t(apiErrors.model)}</p>{/if}
            </div>
            <div class="field">
              <label class="label" for="provider-key">{t("settings.provider.apiKey")}</label>
              <div class="row" style="gap:10px">
                <input
                  class="input mono grow"
                  id="provider-key"
                  type={showKey ? "text" : "password"}
                  bind:value={apiDraft.apiKey}
                  placeholder={t("settings.provider.apiKeyPlaceholder")}
                  spellcheck="false"
                  autocomplete="off"
                  aria-describedby="provider-key-help"
                />
                <button class="btn secondary" type="button" aria-pressed={showKey} onclick={() => (showKey = !showKey)}>{showKey ? t("settings.provider.hide") : t("settings.provider.show")}</button>
              </div>
              <p class="help" id="provider-key-help">{t("settings.provider.apiKeyHelp")}</p>
            </div>
            <button class="btn primary" type="submit" id="btn-save-provider">{t("settings.provider.save")}</button>
          </form>
        {/if}
        <div class="field">
          <label class="check-row">
            <input type="checkbox" checked={settings.plannerCanRead && !usingApi} disabled={usingApi} onchange={(e) => saveControl(e.currentTarget, { plannerCanRead: e.currentTarget.checked }, t("settings.planner.canReadSaved"))} />
            <span>{t("settings.planner.canRead")}</span>
          </label>
          <p class="meta" style="margin:0">{usingApi ? t("settings.planner.readApi") : t("settings.planner.readCli")}</p>
        </div>
        <div class="field" id="auto-run">
          <span class="label">{t("settings.autoRun.title")}</span>
          <p class="help">{t("settings.autoRun.help")}</p>
          {#if settings.autoRunFolders.length}
            <ul class="roots">
              {#each settings.autoRunFolders as f (f)}
                <li>
                  <span class="mono grow" title={f}>{shortPath(f)}</span>
                  <button class="btn ghost sm" type="button" aria-label={t("settings.autoRun.stop", { folder: folderName(f) })} onclick={() => removeAutoRun(f)}>{t("settings.remove")}</button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="meta" style="margin:0">{t("settings.autoRun.none")}</p>
          {/if}
          {#if autoConfirm}
            <div class="warn bypass-warn" role="alert">
              <Warning size={18} aria-hidden="true" />
              <div class="grow">
                <p style="margin:0 0 10px"><b>{t("settings.autoRun.confirmAsk", { folder: folderName(autoConfirm) })}</b> {t("settings.autoRun.confirmWhy")}</p>
                <div class="row" style="gap:10px;flex-wrap:wrap">
                  <button class="btn danger" type="button" onclick={() => allowAutoRun(autoConfirm)}>{t("settings.autoRun.allow")}</button>
                  <button class="btn secondary" type="button" onclick={() => (autoConfirm = "")}>{t("settings.autoRun.keepAsking")}</button>
                </div>
              </div>
            </div>
          {:else}
            <div class="add-rule">
              <label class="sr-only" for="auto-run-folder">{t("settings.autoRun.pickLabel")}</label>
              <select class="select" id="auto-run-folder" bind:value={autoPick}>
                <option value="">{t("settings.chooseFolder")}</option>
                {#each autoAddable as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
              </select>
              <button class="btn secondary sm" type="button" aria-label={t("settings.autoRun.addLabel")} disabled={!autoPick} onclick={() => (autoConfirm = autoPick)}>{t("settings.add")}</button>
              <button class="btn ghost sm" type="button" aria-label={t("settings.autoRun.browseLabel")} onclick={browseAutoRun}>{t("settings.browse")}</button>
            </div>
          {/if}
        </div>
      </section>
    {:else if section === "projects"}
      <section class="card" id="projects" aria-labelledby="settings-title">
        <p class="desc">
          {t("settings.projects.desc")}
          {#if projectCount !== null}{plural(projectCount, "settings.projects.foundOne", "settings.projects.found")}{/if}
        </p>
        <ul class="roots">
          {#each shownRoots as r (r)}
            <li>
              <span class="mono grow" title={r}>{r}</span>
              {#if settings.projectRoots.length}
                <button class="btn ghost sm" type="button" aria-label={t("settings.projects.stop", { path: r })} onclick={() => removeRoot(r)}>{t("settings.remove")}</button>
              {/if}
            </li>
          {/each}
          {#if shownRoots.length === 0}<li class="meta">{t("settings.projects.empty")}</li>{/if}
        </ul>
        {#if !settings.projectRoots.length && shownRoots.length}<p class="meta" style="margin:0">{t("settings.projects.auto")}</p>{/if}
        <button class="btn secondary" type="button" style="align-self:flex-start" onclick={addRoot}>{t("settings.projects.add")}</button>
      </section>
    {:else if section === "notifications"}
      <section class="card" id="notifications" aria-labelledby="settings-title">
        <div class="checks">
          <label><input type="checkbox" checked={settings.notifyWaiting} onchange={(e) => saveControl(e.currentTarget, { notifyWaiting: e.currentTarget.checked })} />{t("settings.notify.waiting")}</label>
          <label><input type="checkbox" checked={settings.notifyDone} onchange={(e) => saveControl(e.currentTarget, { notifyDone: e.currentTarget.checked })} />{t("settings.notify.done")}</label>
          <label><input type="checkbox" checked={settings.notifyError} onchange={(e) => saveControl(e.currentTarget, { notifyError: e.currentTarget.checked })} />{t("settings.notify.error")}</label>
        </div>
        <table class="rules" id="notify-by-cli">
          <caption>{t("settings.notify.byCli")}</caption>
          <thead><tr><th scope="col"><span class="sr-only">{t("settings.notify.cli")}</span></th>{#each NOTICES as n (n.key)}<th scope="col">{t(n.label)}</th>{/each}</tr></thead>
          <tbody>
            {#each ruleClis as c (c.kind)}
              {@const rule = settings.notifyClis[c.kind] ?? ALL}
              <tr>
                <th scope="row">{c.label}</th>
                {#each NOTICES as n (n.key)}
                  <td><input type="checkbox" aria-label="{c.label}: {t(n.label)}" checked={rule[n.key]} onchange={(e) => setCliRule(e.currentTarget, c.kind, n.key)} /></td>
                {/each}
              </tr>
            {:else}
              <tr><td colspan="4" class="meta">{t("settings.notify.noCli")}</td></tr>
            {/each}
          </tbody>
        </table>
        <table class="rules" id="notify-by-folder">
          <caption>{t("settings.notify.byFolder")}</caption>
          {#if ruleFolders.length}
            <thead><tr><th scope="col"><span class="sr-only">{t("settings.notify.folder")}</span></th>{#each NOTICES as n (n.key)}<th scope="col">{t(n.label)}</th>{/each}<th scope="col"><span class="sr-only">{t("settings.remove")}</span></th></tr></thead>
          {/if}
          <tbody>
            {#each ruleFolders as f (f)}
              {@const rule = settings.notifyProjects[f]}
              <tr>
                <th scope="row"><span class="mono" title={f}>{folderName(f)}</span></th>
                {#each NOTICES as n (n.key)}
                  <td><input type="checkbox" aria-label="{folderName(f)}: {t(n.label)}" checked={rule[n.key]} onchange={(e) => setFolderRule(e.currentTarget, f, n.key)} /></td>
                {/each}
                <td><button class="btn ghost sm" type="button" aria-label={t("settings.notify.removeRule", { folder: folderName(f) })} onclick={() => removeFolderRule(f)}>{t("settings.remove")}</button></td>
              </tr>
            {:else}
              <tr><td colspan="5" class="meta">{t("settings.notify.noFolders")}</td></tr>
            {/each}
          </tbody>
        </table>
        <div class="add-rule">
          <label class="sr-only" for="notify-folder">{t("settings.notify.pickLabel")}</label>
          <select class="select" id="notify-folder" bind:value={folderPick}>
            <option value="">{t("settings.chooseFolder")}</option>
            {#each addable as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
          </select>
          <button class="btn secondary sm" type="button" aria-label={t("settings.notify.addLabel")} disabled={!folderPick} onclick={() => addFolderRule(folderPick)}>{t("settings.add")}</button>
          <button class="btn ghost sm" type="button" aria-label={t("settings.notify.browseLabel")} onclick={browseFolderRule}>{t("settings.browse")}</button>
        </div>
        <p class="meta" style="margin:0">{t("settings.notify.footer")}</p>
      </section>
    {:else if section === "text-size"}
      <section class="card" id="text-size" aria-labelledby="settings-title">
        <p class="desc">{t("settings.text.desc")}</p>
        <fieldset class="bare">
          <legend class="sr-only">{t("settings.text.title")}</legend>
          <div class="sizes">
            {#each TEXT_SIZES as s (s)}
              <label class="opt stack">
                <input type="radio" name="text-size" value={s} bind:group={sizeChoice} onchange={pickSize} />
                <span class="sample" style="font-size:{(15 * s) / 100}px" aria-hidden="true">A</span>
                <b>{s}%</b>
                {#if s === 100}<small>{t("settings.text.default")}</small>{/if}
              </label>
            {/each}
          </div>
        </fieldset>
      </section>
    {:else if section === "theme"}
      <section class="card" id="theme" aria-labelledby="settings-title">
        <p class="desc">{t("settings.theme.desc")}</p>
        <fieldset class="bare">
          <legend class="sr-only">{t("settings.theme.title")}</legend>
          <div class="opts">
            <label class="opt">
              <input type="radio" name="theme" value="light" checked={currentTheme() === "light"} onchange={() => setTheme("light")} />
              <Sun size={18} aria-hidden="true" /><b>{t("settings.theme.day")}</b>
            </label>
            <label class="opt">
              <input type="radio" name="theme" value="dark" checked={currentTheme() === "dark"} onchange={() => setTheme("dark")} />
              <Moon size={18} aria-hidden="true" /><b>{t("settings.theme.dusk")}</b>
            </label>
          </div>
        </fieldset>
      </section>
    {:else if section === "language"}
      <section class="card" id="language" aria-labelledby="settings-title">
        <div class="field">
          <label class="meta" for="lang-select">{t("settings.language.label")}</label>
          <!-- Each language is named in itself; the lang attribute lets a screen reader pronounce it. -->
          <select class="select" id="lang-select" value={settings.language} aria-describedby="lang-help" onchange={(e) => pickLanguage(e.currentTarget)}>
            {#each LANGS as l (l.id)}<option value={l.id} lang={l.id}>{l.label}</option>{/each}
          </select>
          <p class="meta" id="lang-help" style="margin:0">{t("settings.language.help")}</p>
        </div>
      </section>
    {:else if section === "window"}
      <section class="card" id="closing" aria-labelledby="settings-title">
        <div class="field">
          <label class="check-row">
            <input
              type="checkbox"
              checked={settings.closeToTray}
              onchange={(e) =>
                saveControl(e.currentTarget, { closeToTray: e.currentTarget.checked }, e.currentTarget.checked ? t("settings.window.trayOnSaved") : t("settings.window.trayOffSaved"))}
            />
            <span>{t("settings.window.tray")}</span>
          </label>
          <p class="meta" style="margin:0">{settings.closeToTray ? t("settings.window.trayOn") : t("settings.window.trayOff")}</p>
        </div>
        {#if info?.canStartAtLogin}
          <label class="check-row">
            <input
              type="checkbox"
              checked={settings.startAtLogin}
              onchange={(e) =>
                saveControl(
                  e.currentTarget,
                  { startAtLogin: e.currentTarget.checked },
                  e.currentTarget.checked ? t("settings.window.loginOnSaved") : t("settings.window.loginOffSaved"),
                )}
            />
            <span>{t("settings.window.login")}</span>
          </label>
        {/if}
      </section>
    {:else if section === "outside"}
      <section class="card" id="outside" aria-labelledby="settings-title">
        <div class="field">
          <label class="meta" for="scan-select">{t("settings.scan.label")}</label>
          <select class="select" id="scan-select" value={String(settings.scanSeconds)} onchange={(e) => saveControl(e.currentTarget, { scanSeconds: Number(e.currentTarget.value) }, t("settings.scan.saved"))}>
            <option value="5">{t("settings.scan.everySeconds", { n: 5 })}</option>
            <option value="10">{t("settings.scan.everySeconds", { n: 10 })}</option>
            <option value="30">{t("settings.scan.everySeconds", { n: 30 })}</option>
            <option value="60">{t("settings.scan.everyMinute")}</option>
          </select>
        </div>
      </section>
    {:else if section === "history"}
      <section class="card" id="history" aria-labelledby="settings-title">
        <p class="desc">
          {info ? t("settings.history.whereDir", { dir: info.dataDir }) : t("settings.history.where")}
          {#if info}{plural(info.counts.sessions, "settings.history.countOne", "settings.history.count")}{/if}
        </p>
        <div class="field">
          <label class="meta" for="keep-select">{t("settings.history.keepLabel")}</label>
          <select class="select" id="keep-select" value={String(settings.keepDays)} onchange={(e) => pickKeep(e.currentTarget)}>
            {#each KEEP as days (days)}<option value={String(days)}>{keepLabel(days)}</option>{/each}
          </select>
          <p class="meta" style="margin:0">{t("settings.history.olderNote")}</p>
        </div>
        <button class="btn secondary" type="button" style="align-self:flex-start" onclick={clearHistory}>
          {clearStep ? t("settings.history.deleteAgain") : t("settings.history.delete")}
        </button>
        <div class="field" id="shell-commands">
          <span class="label">{t("settings.commands.title")}</span>
          <label class="check-row">
            <input
              type="checkbox"
              checked={settings.shellSuggestions}
              onchange={(e) =>
                saveControl(
                  e.currentTarget,
                  { shellSuggestions: e.currentTarget.checked },
                  e.currentTarget.checked ? t("settings.commands.onSaved") : t("settings.commands.offSaved"),
                )}
            />
            <span>{t("settings.commands.suggest")}</span>
          </label>
          <p class="meta" style="margin:0">{settings.shellSuggestions ? t("settings.commands.suggestOn") : t("settings.commands.suggestOff")}</p>
          <p class="help">{t("settings.commands.saved")}</p>
          <button class="btn secondary" type="button" style="align-self:flex-start" onclick={deleteCommands}>
            {commandsStep ? t("settings.commands.deleteAgain") : t("settings.commands.delete")}
          </button>
        </div>
      </section>
    {/if}
    {#if info}<p class="meta note" style="margin:0">OpenCompanion {info.version}</p>{/if}
  {/if}
</main>

<style>
  /* One column wide enough for the permission table without a scrollbar; past it the painting shows. */
  .settings-main > :global(*) {
    max-width: 960px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  /* A card is as wide as the column; its short controls keep the width they had in the old two-column grid. */
  .card .select,
  .card .opts,
  .provider {
    max-width: 480px;
  }
  .card .sizes {
    max-width: 560px;
  }
  .card .rules {
    max-width: 640px;
  }
  .desc {
    margin: 0;
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
    min-width: 880px;
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
  }
</style>
