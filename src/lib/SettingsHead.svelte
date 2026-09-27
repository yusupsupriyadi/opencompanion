<script lang="ts">
  import { page } from "$app/state";
  import Books from "phosphor-svelte/lib/Books";
  import SlidersHorizontal from "phosphor-svelte/lib/SlidersHorizontal";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import type { Snippet } from "svelte";
  import { t } from "./i18n.svelte";

  // The part of the page head that is the same on every Settings screen; `children` goes at the
  // far end, for the screen's own action (Rescan).
  let { children }: { children?: Snippet } = $props();

  const TABS = [
    { href: "/settings", key: "settings.tabs.general", icon: SlidersHorizontal },
    { href: "/settings/clis", key: "work.clis.heading", icon: TerminalWindow },
    { href: "/settings/skills", key: "work.skills.heading", icon: Books },
  ] as const;

  const path = $derived(page.url.pathname.replace(/\/+$/, "") || "/");
</script>

<header class="page-head settings-head">
  <h1>{t("settings.title")}</h1>
  <nav class="seg" id="settings-tabs" aria-label={t("settings.tabs.label")}>
    {#each TABS as tab (tab.href)}
      <a href={tab.href} aria-current={path === tab.href ? "page" : undefined}>
        <tab.icon size={16} aria-hidden="true" /><span>{t(tab.key)}</span>
      </a>
    {/each}
  </nav>
  {#if children}<div class="head-actions">{@render children()}</div>{/if}
</header>

<style>
  .settings-head {
    flex-wrap: wrap;
    gap: 12px 24px;
  }
  .head-actions {
    margin-left: auto;
  }
</style>
