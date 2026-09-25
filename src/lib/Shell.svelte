<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import DeleteSessionDialog from "./DeleteSessionDialog.svelte";
  import NewSessionDialog from "./NewSessionDialog.svelte";
  import Sidebar from "./Sidebar.svelte";
  import TitleBar from "./TitleBar.svelte";
  import Toast from "./Toast.svelte";
  import { app, initTheme, startDesktop } from "./store.svelte";

  let { children }: { children: Snippet } = $props();

  const onboarding = $derived(page.url.pathname === "/onboarding");

  onMount(() => {
    initTheme();
    startDesktop().then(() => {
      if (app.settings && !app.settings.onboarded && page.url.pathname !== "/onboarding") goto("/onboarding");
    });
  });
</script>

{#if onboarding}
  {@render children()}
{:else}
  <div class="app">
    <Sidebar />
    {@render children()}
  </div>
  <DeleteSessionDialog />
  <NewSessionDialog />
{/if}
<!-- Last in the DOM so Tab reaches the page first; CSS pins it to the top of the window. -->
<TitleBar />
<Toast />
