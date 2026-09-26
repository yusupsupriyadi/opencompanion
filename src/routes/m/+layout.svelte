<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import House from "phosphor-svelte/lib/House";
  import Kanban from "phosphor-svelte/lib/Kanban";
  import { onMount } from "svelte";
  import "$lib/phone.css";
  import { connect, getToken, loadSessions, phone, reconnectNow } from "$lib/phone.svelte";

  let { children } = $props();
  const onPair = $derived(page.url.pathname.startsWith("/m/pair"));

  // The three top-level screens get the tab bar; detail screens and forms own the bottom edge instead.
  const TABS = [
    { href: "/m", label: "Sessions", icon: House },
    { href: "/m/chat", label: "Chat", icon: ChatsCircle },
    { href: "/m/board", label: "Board", icon: Kanban },
  ];
  const path = $derived(page.url.pathname.replace(/\/$/, "") || "/m");
  const tabbed = $derived(TABS.some((t) => t.href === path));
  const waiting = $derived(phone.sessions.filter((s) => s.status === "waiting").length);
  const offline = $derived(phone.connection === "offline" && !onPair);

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

<!-- The screen underneath stays mounted while the desktop is away, so a half-written form or chat survives. -->
<div class="phone" class:has-tabs={tabbed} inert={offline}>
  {@render children()}
  {#if tabbed}
    <nav class="tabs" id="phone-tabs" aria-label="Phone sections">
      <div>
        {#each TABS as t (t.href)}
          <a href={t.href} aria-current={path === t.href ? "page" : undefined}>
            <t.icon size={22} weight={path === t.href ? "fill" : "regular"} aria-hidden="true" />
            {t.label}
            {#if t.href === "/m" && waiting > 0}<span class="tab-count">{waiting}<span class="sr-only"> waiting for you</span></span>{/if}
          </a>
        {/each}
      </div>
    </nav>
  {/if}
</div>
{#if offline}
  <div class="phone offline-layer" id="phone-offline-layer">
    <header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
    <main class="content" id="phone-offline">
      <img class="art" src="/meadow-day.png" alt="" aria-hidden="true" />
      <h1 class="m-h1">Can't reach your desktop</h1>
      <p class="m-p">This phone is paired, but your computer did not answer. Things to check:</p>
      <ol class="try">
        <li>The computer is awake and OpenCompanion is open.</li>
        <li>Phone access is still on in OpenCompanion Settings.</li>
        <li>This phone is on the same Wi-Fi, or on your VPN.</li>
      </ol>
      <button class="btn primary block" type="button" onclick={reconnectNow}><ArrowClockwise size={16} aria-hidden="true" />Try again</button>
      <p class="small" style="margin:0">Trying again by itself every few seconds. What you were typing is kept.</p>
    </main>
  </div>
{/if}
<!-- Always in the page, so screen readers hear the text when it arrives. -->
<div class="toast" class:show={phone.notice} role="status" aria-live="polite">{phone.notice}</div>
