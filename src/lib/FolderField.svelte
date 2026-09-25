<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import { onMount } from "svelte";
  import { api } from "./api";
  import { folderName } from "./format";

  let {
    value = $bindable(""),
    error = "",
    id,
    label = "Project folder",
    oninput,
  }: { value?: string; error?: string; id: string; label?: string; oninput?: () => void } = $props();

  // Folders used in OpenCompanion first; on a fresh install, the projects found on disk.
  let suggestions = $state<string[]>([]);
  let suggestionLabel = $state("Recent:");

  onMount(async () => {
    try {
      const recent = (await api.recentProjects()).slice(0, 5).map((p) => p.path);
      if (recent.length) {
        suggestions = recent;
        return;
      }
      suggestions = (await api.projectFolders()).slice(0, 6).map((p) => p.path);
      suggestionLabel = "Projects:";
    } catch {
      suggestions = [];
    }
  });

  async function browse() {
    const picked = await openDialog({ directory: true, multiple: false, defaultPath: value || undefined, title: "Choose a project folder" });
    if (typeof picked === "string") {
      value = picked;
      oninput?.();
    }
  }
</script>

<div class="field">
  <label class="label" for={id}>{label}</label>
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
        placeholder="C:\Users\you\Project\my-app"
      />
    </div>
    <button class="btn secondary" type="button" onclick={browse}>Browse</button>
  </div>
  {#if error}<p class="error" id="{id}-error">{error}</p>{/if}
  {#if suggestions.length}
    <div class="tags">
      <span>{suggestionLabel}</span>
      {#each suggestions as p (p)}
        <button class="tag" type="button" title={p} onclick={() => { value = p; oninput?.(); }}>{folderName(p)}</button>
      {/each}
    </div>
  {/if}
</div>
