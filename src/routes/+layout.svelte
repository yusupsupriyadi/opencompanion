<script lang="ts">
  import "@fontsource/nunito/400.css";
  import "@fontsource/nunito/600.css";
  import "@fontsource/nunito/700.css";
  import "@fontsource/nunito/800.css";
  import "@fontsource/ibm-plex-mono/400.css";
  import "@fontsource/ibm-plex-mono/400-italic.css";
  import "@fontsource/ibm-plex-mono/600.css";
  import "@fontsource/pixelify-sans/400.css";
  import "../app.css";
  import { page } from "$app/state";
  import Shell from "$lib/Shell.svelte";

  let { children } = $props();

  // `/m` is the phone companion, served over the LAN: no desktop shell, no Tauri APIs.
  const phone = $derived(page.url.pathname === "/m" || page.url.pathname.startsWith("/m/"));
</script>

{#if phone}
  {@render children()}
{:else}
  <Shell>{@render children()}</Shell>
{/if}
