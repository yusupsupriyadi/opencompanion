<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import House from "phosphor-svelte/lib/House";
  import { onMount } from "svelte";
  import "$lib/phone.css";
  import AppLogo from "$lib/AppLogo.svelte";
  import { setLang, t } from "$lib/i18n.svelte";
  import { connect, getToken, loadSessions, phone, reconnectNow, registerWorker } from "$lib/phone.svelte";

  let { children } = $props();
  const onPair = $derived(page.url.pathname.startsWith("/m/pair"));

  // The two top-level screens get the tab bar; detail screens and forms own the bottom edge instead.
  // The labels are getters, so they follow the UI language.
  const TABS = [
    { href: "/m", get label() { return t("phone.nav.sessions"); }, icon: House },
    { href: "/m/chat", get label() { return t("phone.nav.chat"); }, icon: ChatsCircle },
  ];
  const path = $derived(page.url.pathname.replace(/\/$/, "") || "/m");
  const tabbed = $derived(TABS.some((tab) => tab.href === path));
  const waiting = $derived(phone.sessions.filter((s) => s.status === "waiting").length);
  const offline = $derived(phone.connection === "offline" && !onPair);

  // The phone draws to the screen edges and keeps clear of the notch and home indicator with the
  // safe-area insets in phone.css. The desktop window keeps app.html's viewport as it is.
  onMount(() => {
    const viewport = document.querySelector<HTMLMetaElement>('meta[name="viewport"]');
    const before = viewport?.content;
    if (viewport) viewport.content = "width=device-width, initial-scale=1, viewport-fit=cover";
    return () => {
      if (viewport && before !== undefined) viewport.content = before;
    };
  });

  // iOS shows :active, the pressed look of keys, buttons and cards, only where a touch listener is.
  onMount(() => {
    const none = () => undefined;
    document.addEventListener("touchstart", none, { passive: true });
    return () => document.removeEventListener("touchstart", none);
  });

  onMount(() => {
    registerWorker();
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

<!-- Lets the phone add this page to its home screen and open it without browser bars. The status bar
     stays "default": "black-translucent" would put white status text over the cream Day theme. -->
<svelte:head>
  <link rel="manifest" href="/m/manifest.webmanifest" />
  <meta name="theme-color" content="#FBF3CF" media="(prefers-color-scheme: light)" />
  <meta name="theme-color" content="#1B1F1A" media="(prefers-color-scheme: dark)" />
  <meta name="mobile-web-app-capable" content="yes" />
  <meta name="apple-mobile-web-app-capable" content="yes" />
  <meta name="apple-mobile-web-app-status-bar-style" content="default" />
  <meta name="apple-mobile-web-app-title" content="OpenCompanion" />
  <link rel="apple-touch-icon" href="/m/icons/apple-touch-icon.png" />
</svelte:head>

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
    <header class="bar"><span class="brand grow"><AppLogo />OpenCompanion</span></header>
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
