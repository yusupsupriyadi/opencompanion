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
  import { setLang, t } from "$lib/i18n.svelte";
  import { connect, getToken, loadSessions, phone, reconnectNow } from "$lib/phone.svelte";

  let { children } = $props();
  const onPair = $derived(page.url.pathname.startsWith("/m/pair"));

  // The three top-level screens get the tab bar; detail screens and forms own the bottom edge instead.
  // The labels are getters, so they follow the UI language.
  const TABS = [
    { href: "/m", get label() { return t("phone.nav.sessions"); }, icon: House },
    { href: "/m/chat", get label() { return t("phone.nav.chat"); }, icon: ChatsCircle },
    { href: "/m/board", get label() { return t("phone.nav.board"); }, icon: Kanban },
  ];
  const path = $derived(page.url.pathname.replace(/\/$/, "") || "/m");
  const tabbed = $derived(TABS.some((tab) => tab.href === path));
  const waiting = $derived(phone.sessions.filter((s) => s.status === "waiting").length);
  const offline = $derived(phone.connection === "offline" && !onPair);

  onMount(() => {
    // The phone speaks the desktop's UI language; asking needs no pairing.
    fetch("/api/hello")
      .then((r) => r.json())
      .then((h) => setLang(h.language))
      .catch(() => undefined);
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
    <nav class="tabs" id="phone-tabs" aria-label={t("phone.nav.label")}>
      <div>
        {#each TABS as tab (tab.href)}
          <a href={tab.href} aria-current={path === tab.href ? "page" : undefined}>
            <tab.icon size={22} weight={path === tab.href ? "fill" : "regular"} aria-hidden="true" />
            {tab.label}
            {#if tab.href === "/m" && waiting > 0}<span class="tab-count">{waiting}<span class="sr-only"> {t("phone.nav.waiting")}</span></span>{/if}
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
      <h1 class="m-h1">{t("phone.offline.title")}</h1>
      <p class="m-p">{t("phone.offline.lead")}</p>
      <ol class="try">
        <li>{t("phone.offline.awake")}</li>
        <li>{t("phone.offline.access")}</li>
        <li>{t("phone.offline.network")}</li>
      </ol>
      <button class="btn primary block" type="button" onclick={reconnectNow}><ArrowClockwise size={16} aria-hidden="true" />{t("phone.tryAgain")}</button>
      <p class="small" style="margin:0">{t("phone.offline.retrying")}</p>
    </main>
  </div>
{/if}
<!-- Always in the page, so screen readers hear the text when it arrives. -->
<div class="toast" class:show={phone.notice} role="status" aria-live="polite">{phone.notice}</div>
