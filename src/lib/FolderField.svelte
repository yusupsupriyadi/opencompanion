<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import { onMount } from "svelte";
  import { api } from "./api";
  import { folderName } from "./format";
  import { t } from "./i18n.svelte";
  import { folderExample } from "./platform";

  // `label` has no default value: the default name, "Project folder", has to follow the UI language.
  let {
    value = $bindable(""),
    error = "",
    id,
    label,
    oninput,
  }: { value?: string; error?: string; id: string; label?: string; oninput?: () => void } = $props();

  // Folders used in OpenCompanion first; on a fresh install, the projects found on disk.
  let suggestions = $state<string[]>([]);
  let fromDisk = $state(false);

  onMount(async () => {
    try {
      const recent = (await api.recentProjects()).slice(0, 5).map((p) => p.path);
      if (recent.length) {
        suggestions = recent;
        return;
      }
      suggestions = (await api.projectFolders()).slice(0, 6).map((p) => p.path);
      fromDisk = true;
    } catch {
      suggestions = [];
    }
  });

  async function browse() {
    const picked = await openDialog({ directory: true, multiple: false, defaultPath: value || undefined, title: t("shell.folder.pickerTitle") });
    if (typeof picked === "string") {
      value = picked;
      oninput?.();
    }
  }
</script>

<div class="field">
  <label class="label" for={id}>{label ?? t("shell.folder.label")}</label>
  <div class="row" style="gap:10px">
    <div class="input-wrap grow">
      <FolderSimple size={18} aria-hidden="true" />
      <input
        {id}
        bind:value
        oninput={() => oninput?.()}
        aria-invalid={error ? "true" : undefined}
        aria-describedby={error ? `${id}-error` : undefined}
        autocomplete="off"
        spellcheck="false"
        placeholder={folderExample()}
      />
    </div>
    <button class="btn secondary" type="button" onclick={browse}>{t("shell.folder.browse")}</button>
  </div>
  {#if error}<p class="error" id="{id}-error">{error}</p>{/if}
  {#if suggestions.length}
    <div class="tags">
      <span>{fromDisk ? t("shell.folder.projects") : t("shell.folder.recent")}</span>
      {#each suggestions as p (p)}
        <button class="tag" type="button" title={p} onclick={() => { value = p; oninput?.(); }}>{folderName(p)}</button>
      {/each}
    </div>
  {/if}
</div>
