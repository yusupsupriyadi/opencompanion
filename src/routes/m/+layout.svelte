<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import { onMount } from "svelte";
  import "$lib/phone.css";
  import { connect, getToken, loadSessions, phone, reconnectNow } from "$lib/phone.svelte";

  let { children } = $props();
  const onPair = $derived(page.url.pathname.startsWith("/m/pair"));

  onMount(() => {
    if (!getToken()) {
      if (!onPair) goto(`/m/pair${page.url.search}`);
      return;
    }
    loadSessions().catch(() => undefined);
    connect();
  });

  $effect(() => {
    if (phone.unpaired && !page.url.pathname.startsWith("/m/pair")) goto("/m/pair");
  });
</script>

<svelte:head><meta name="theme-color" content="#FBF3CF" /></svelte:head>

<div class="phone">
  {#if phone.connection === "offline" && !onPair}
    <header class="bar"><span class="brand grow">AI Remote <Cloud size={20} aria-hidden="true" /></span></header>
    <main class="content" id="phone-offline">
      <img class="art" src="/meadow-day.webp" alt="" aria-hidden="true" />
      <h1 class="m-h1">Can't reach your desktop</h1>
      <p class="m-p">This phone is paired, but your computer did not answer. Things to check:</p>
      <ol class="try">
        <li>The computer is awake and AI Remote is open.</li>
        <li>Phone access is still on in AI Remote Settings.</li>
        <li>This phone is on the same Wi-Fi, or on your VPN.</li>
      </ol>
      <button class="btn primary block" type="button" onclick={reconnectNow}><ArrowClockwise size={16} aria-hidden="true" />Try again</button>
      <p class="small" style="margin:0">Trying again by itself every few seconds.</p>
    </main>
  {:else}
    {@render children()}
  {/if}
</div>
{#if phone.notice}<div class="toast show" role="status" aria-live="polite">{phone.notice}</div>{/if}
