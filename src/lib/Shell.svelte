<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import DeleteSessionDialog from "./DeleteSessionDialog.svelte";
  import NewSessionDialog from "./NewSessionDialog.svelte";
  import SettingsSidebar from "./SettingsSidebar.svelte";
  import Sidebar from "./Sidebar.svelte";
  import TitleBar from "./TitleBar.svelte";
  import Toast from "./Toast.svelte";
  import { app, initTheme, startDesktop } from "./store.svelte";
  import UpdateDialog from "./UpdateDialog.svelte";
  import { startUpdates } from "./update.svelte";

  let { children }: { children: Snippet } = $props();

  const onboarding = $derived(page.url.pathname === "/onboarding");
  const inSettings = $derived(page.url.pathname === "/settings" || page.url.pathname.startsWith("/settings/"));

  // Settings swaps the app sidebar for its own, whose Back link returns to the screen open before it.
  let lastApp = $state("/");
  $effect(() => {
    if (!inSettings && !onboarding) lastApp = page.url.pathname + page.url.search;
  });

  onMount(() => {
    initTheme();
    startDesktop().then(() => {
      if (app.settings && !app.settings.onboarded && page.url.pathname !== "/onboarding") goto("/onboarding");
    });
    startUpdates();
    // A click on a desktop notification opens the session it is about (PRD FR-40).
    const un = listen<string>("open-session", (e) => goto(`/session?id=${encodeURIComponent(e.payload)}`));
    return () => {
      un.then((f) => f());
    };
  });
</script>

{#if onboarding}
  {@render children()}
{:else}
  <div class="app" class:in-settings={inSettings}>
    {#if inSettings}<SettingsSidebar back={lastApp} />{:else}<Sidebar />{/if}
    {@render children()}
  </div>
  <DeleteSessionDialog />
  <NewSessionDialog />
  <UpdateDialog />
  <ContextMenu />
{/if}
<!-- Last in the DOM so Tab reaches the page first; CSS pins it to the top of the window. -->
<TitleBar />
<Toast />
