<script lang="ts">
  import { page } from "$app/state";
  import type { Snippet } from "svelte";
  import { t } from "./i18n.svelte";
  import { currentItem } from "./settings-nav";

  // The head of every Settings screen: the name of the item picked in the Settings sidebar, and
  // `children` at the far end for the screen's own action (Rescan). Sections label themselves
  // with its id.
  let { children }: { children?: Snippet } = $props();

  const item = $derived(currentItem(page.url));
</script>

<header class="page-head settings-head">
  <h1 id="settings-title">{t(item.key)}</h1>
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
