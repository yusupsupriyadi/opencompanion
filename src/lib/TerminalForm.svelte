<script lang="ts">
  import TerminalIcon from "phosphor-svelte/lib/Terminal";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import { api, errorText, type ShellInfo } from "./api";
  import { folderName } from "./format";
  import { t, tb } from "./i18n.svelte";

  // A shell always opens in the session's folder; only the shell is picked here. `split` opens it in the shown tab.
  let {
    folder,
    split = false,
    onsubmit,
    oncancel,
  }: { folder: string; split?: boolean; onsubmit: (shell: string) => Promise<void>; oncancel: () => void } = $props();

  // The dialog remounts the form every time it opens.
  let shells = $state<ShellInfo[]>([]);
  let shellsState = $state<"loading" | "ready" | "error">("loading");
  let shell = $state("");
  let failure = $state("");
  let busy = $state(false);

  const picked = $derived(shells.find((s) => s.id === shell));

  onMount(async () => {
    try {
      shells = await api.terminalShells();
      // Every terminal starts in the default shell, PowerShell on Windows, whatever was picked last.
      shell = shells[0]?.id ?? "";
      shellsState = "ready";
    } catch (e) {
      failure = errorText(e);
      shellsState = "error";
    }
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    busy = true;
    try {
      await onsubmit(shell);
    } catch (err) {
      failure = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="d-body" novalidate onsubmit={submit}>
  <div class="row">
    <h2 id="nt-title" class="grow">{split ? t("terminal.split") : t("terminal.new")}</h2>
    <button class="icon-btn" type="button" aria-label={t("shell.close")} onclick={oncancel}><X size={18} aria-hidden="true" /></button>
  </div>

  <div class="field">
    <p class="label">{t("shell.folder.label")}</p>
    <p class="mono" id="nt-folder">{folder}</p>
  </div>

  <div class="field">
    {#if shells.length}
      <label class="label" for="nt-shell">{t("terminal.form.shell")}</label>
      <select class="select" id="nt-shell" bind:value={shell} aria-describedby="nt-shell-path nt-shell-help">
        {#each shells as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
      </select>
      <p class="help mono" id="nt-shell-path">{picked?.path ?? ""}</p>
    {:else}
      <p class="label">{t("terminal.form.shell")}</p>
      {#if shellsState === "loading"}
        <p class="help" role="status">{t("terminal.form.shellsLoading")}</p>
      {:else if shellsState === "ready"}
        <p class="error">{t("terminal.form.noShell")}</p>
      {/if}
    {/if}
    <p class="help" id="nt-shell-help">{t("terminal.form.help")}</p>
  </div>

  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  <div class="d-foot">
    <span class="meta grow">{t("shell.form.escHint")}</span>
    <button class="btn secondary" type="button" onclick={oncancel}>{t("shell.form.cancel")}</button>
    <button class="btn primary" type="submit" id="btn-open-terminal" disabled={busy || !shell}>
      <TerminalIcon size={16} aria-hidden="true" /><span>{busy ? t("terminal.form.opening") : t("terminal.form.submit", { folder: folderName(folder) })}</span>
    </button>
  </div>
</form>

<style>
  #nt-folder,
  #nt-shell-path {
    overflow-wrap: anywhere;
  }
  #nt-folder {
    margin: 0;
    font-size: 13px;
  }
</style>
