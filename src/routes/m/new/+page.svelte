<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import Play from "phosphor-svelte/lib/Play";
  import { onMount, tick } from "svelte";
  import type { CliKind, Mode, PermMode, SessionInfo, Task } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import { CLI_LABEL, MODES, folderName, shortPath } from "$lib/format";
  import { call, type PhoneOptions } from "$lib/phone.svelte";

  // `?task=` runs a Board card; `?cwd=` starts in that folder.
  const taskId = page.url.searchParams.get("task");
  const OTHER = "__other";

  let options = $state<PhoneOptions | null>(null);
  let task = $state<Task | null>(null);
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
  const label = $derived(`${task ? "Run" : `Start ${CLI_LABEL[cli]}`} in ${cwd ? folderName(cwd) : "…"}`);

  async function load() {
    loadState = "loading";
    try {
      const [o, t] = await Promise.all([
        call<PhoneOptions>("/api/options"),
        taskId ? call<{ tasks: Task[] }>("/api/tasks").then((r) => r.tasks.find((x) => x.id === taskId) ?? null) : null,
      ]);
      if (taskId && !t) {
        loadState = "error";
        loadError = "This card is no longer on the Board.";
        return;
      }
      options = o;
      task = t;
      const usable = o.clis.filter((c) => c.path);
      cli = usable.find((c) => c.kind === t?.cli)?.kind ?? usable.find((c) => c.kind !== "gemini")?.kind ?? usable[0]?.kind ?? "claude";
      const start = t?.project || page.url.searchParams.get("cwd") || "";
      if (start && !o.folders.some((f) => f.path === start)) {
        folder = OTHER;
        typed = start;
      } else {
        folder = start || o.folders[0]?.path || OTHER;
      }
      prompt = t ? t.notes || t.title : "";
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
      folderError = "Choose the project folder.";
      focus(folder === OTHER ? "pn-path" : "pn-folder");
      return;
    }
    if (headless && !prompt.trim()) {
      promptError = "Headless sessions need a prompt.";
      focus("pn-prompt");
      return;
    }
    if (headless && cli === "gemini") {
      failure = "Headless mode for Gemini CLI is not supported yet. Pick Interactive.";
      return;
    }
    busy = true;
    try {
      const body = JSON.stringify({ cli, cwd, mode, prompt: prompt.trim(), permissionMode });
      const path = task ? `/api/tasks/${task.id}/run` : "/api/sessions";
      const r = await call<{ session: SessionInfo }>(path, { method: "POST", body });
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

<svelte:head><title>{task ? "Run card" : "New session"} · OpenCompanion</title></svelte:head>

<header class="bar">
  <a class="back" href={taskId ? "/m/board" : "/m"}><CaretLeft size={20} aria-hidden="true" />{taskId ? "Board" : "Sessions"}</a>
</header>
<main class="content" id="phone-new-session">
  <h1 class="m-h1" style="overflow-wrap:anywhere">{task ? task.title : "New session"}</h1>

  {#if loadState === "loading"}
    <p class="m-p" role="status">Checking the CLIs and project folders on your computer…</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{loadError}</p>
    <button class="btn secondary block" type="button" onclick={load}>Try again</button>
  {:else if options}
    <p class="m-p">
      {task ? "The session starts on your computer and the card moves to In progress." : "The session starts on your computer. You follow it from here or from the desktop."}
    </p>
    <form class="m-form" novalidate onsubmit={submit}>
      <fieldset>
        <legend>CLI</legend>
        {#if installed.length === 0}
          <p class="error">No supported CLI was found on your computer. Install one there, then press Rescan on the CLIs screen.</p>
        {/if}
        <div class="opts">
          {#each options.clis as c (c.kind)}
            <label class="opt">
              <input type="radio" name="pn-cli" value={c.kind} bind:group={cli} disabled={!c.path} />
              <CliMark kind={c.kind} />
              <span><b>{c.label}</b><small class={c.path ? "mono" : ""}>{c.path ? (c.version ?? "version unknown") : "Not installed"}</small></span>
              <span class="check"><CheckCircle size={18} aria-hidden="true" /></span>
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="field">
        <label class="label" for="pn-folder">Project folder</label>
        <select
          class="select"
          id="pn-folder"
          bind:value={folder}
          onchange={() => (folderError = "")}
          aria-invalid={folderError && folder !== OTHER ? "true" : undefined}
          aria-describedby="pn-folder-note"
        >
          {#each options.folders as f (f.path)}<option value={f.path}>{f.name} · {shortPath(f.path)}</option>{/each}
          <option value={OTHER}>Another folder…</option>
        </select>
        {#if folder === OTHER}
          <label class="sr-only" for="pn-path">Folder path</label>
          <input
            class="input mono"
            id="pn-path"
            bind:value={typed}
            oninput={() => (folderError = "")}
            placeholder="Full path of the folder on your computer"
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
          <p class="help" id="pn-folder-note">Folders OpenCompanion found on your computer, most recent first.</p>
        {/if}
      </div>

      <fieldset>
        <legend>Mode</legend>
        <div class="opts">
          <label class="opt">
            <input type="radio" name="pn-mode" value="headless" bind:group={mode} />
            <span><b>Headless</b><small>Runs the prompt and shows each step here. Follow-ups continue the same conversation.</small></span>
            <span class="check"><CheckCircle size={18} aria-hidden="true" /></span>
          </label>
          <label class="opt">
            <input type="radio" name="pn-mode" value="interactive" bind:group={mode} />
            <span><b>Interactive</b><small>A terminal on your computer. Here you see its screen, type into it and press keys.</small></span>
            <span class="check"><CheckCircle size={18} aria-hidden="true" /></span>
          </label>
        </div>
      </fieldset>

      <div class="field">
        <label class="label" for="pn-prompt">{headless ? "Prompt" : "First message (optional)"}</label>
        <textarea
          class="textarea"
          id="pn-prompt"
          bind:value={prompt}
          oninput={() => (promptError = "")}
          aria-invalid={promptError ? "true" : undefined}
          aria-describedby={promptError ? "pn-prompt-error" : undefined}
          placeholder={headless ? `What should ${CLI_LABEL[cli]} do?` : `Sent to ${CLI_LABEL[cli]} as soon as the terminal opens.`}
        ></textarea>
        {#if promptError}<p class="error" id="pn-prompt-error">{promptError}</p>{/if}
      </div>

      <div class="field">
        <label class="label" for="pn-perm">Permission mode</label>
        <select class="select" id="pn-perm" bind:value={permissionMode} aria-describedby="pn-perm-help">
          {#each MODES as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
        </select>
        <p class="help" id="pn-perm-help">{modeInfo.short} The default comes from Settings.</p>
        {#if permissionMode === "bypass"}
          <p class="error" role="note">Bypass: {CLI_LABEL[cli]} can change or delete any file and run any command in this folder without asking.</p>
        {/if}
      </div>

      {#if failure}<p class="err-text" role="alert" style="margin:0">{failure}</p>{/if}

      <button class="btn primary block" type="submit" id="btn-start-session" disabled={busy || installed.length === 0}>
        <Play size={16} aria-hidden="true" /><span style="overflow:hidden;text-overflow:ellipsis">{busy ? "Starting…" : label}</span>
      </button>
    </form>
  {/if}
</main>
