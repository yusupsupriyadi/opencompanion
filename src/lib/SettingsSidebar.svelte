<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ArrowLeft from "phosphor-svelte/lib/ArrowLeft";
  import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
  import { t } from "./i18n.svelte";
  import { currentPlatform } from "./platform";
  import { currentItem, filterGroups } from "./settings-nav";

  // Takes the app sidebar's place on every Settings screen. `back` is the app screen that was open
  // before Settings.
  let { back = "/" }: { back?: string } = $props();

  let query = $state("");
  let search = $state<HTMLInputElement>();
  const groups = $derived(filterGroups(query));
  const current = $derived(currentItem(page.url).id);
  const mac = currentPlatform() === "macos";

  // Enter opens the first match; Escape empties the box before it does anything else.
  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      const first = groups[0]?.items[0];
      if (first) goto(first.href);
    } else if (e.key === "Escape" && query) {
      e.preventDefault();
      query = "";
    }
  }

  // Ctrl+F (⌘F on macOS) jumps to the search, as the hint in the box says.
  function onWindowKey(e: KeyboardEvent) {
    if ((mac ? e.metaKey : e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      search?.focus();
      search?.select();
    }
  }
</script>

<svelte:window onkeydown={onWindowKey} />

<aside class="sidebar set-side" id="settings-sidebar" aria-label={t("settings.title")}>
  <a class="set-back" id="btn-settings-back" href={back}><ArrowLeft size={16} aria-hidden="true" /><span>{t("settings.back")}</span></a>
  <div class="input-wrap set-search">
    <MagnifyingGlass size={16} aria-hidden="true" />
    <input
      type="search"
      id="settings-search"
      bind:this={search}
      bind:value={query}
      aria-label={t("settings.search.label")}
      aria-keyshortcuts={mac ? "Meta+F" : "Control+F"}
      placeholder={t("settings.search.label")}
      spellcheck="false"
      autocomplete="off"
      onkeydown={onSearchKey}
    />
    {#if !query}<kbd aria-hidden="true">{mac ? "⌘F" : "Ctrl F"}</kbd>{/if}
  </div>
  <nav class="set-nav" id="settings-nav" aria-label={t("settings.tabs.label")}>
    {#each groups as g, i (g.key)}
      <div class="set-group">
        <span class="side-label" id="set-group-{i}">{t(g.key)}</span>
        <ul aria-labelledby="set-group-{i}">
          {#each g.items as item (item.id)}
            <li>
              <a href={item.href} aria-current={item.id === current ? "page" : undefined}>
                <item.icon size={18} aria-hidden="true" /><span>{t(item.key)}</span>
              </a>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <p class="set-empty" role="status">{t("settings.search.none", { query: query.trim() })}</p>
    {/each}
  </nav>
</aside>

<style>
  .set-side {
    gap: 16px;
    overflow: hidden;
  }
  .set-back {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
    padding: 6px 12px;
    border-radius: var(--r-btn);
    color: var(--ink-2);
    font-weight: 600;
    transition: background-color 0.12s ease-out;
  }
  .set-back:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .set-search {
    flex: none;
  }
  /* A search for words, not a path: the UI face, not the mono the shared field uses. */
  .set-search input {
    font: 400 13px var(--font-ui);
    padding: 9px 0;
  }
  kbd {
    flex: none;
    padding: 0 6px;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    font: 400 12px/1.5 var(--font-mono);
    color: var(--ink-2);
    white-space: nowrap;
  }
  /* Only the list scrolls, so Back and the search stay in reach. The 4px pad keeps focus rings unclipped. */
  .set-nav {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    margin: 0 -4px -4px;
    padding: 4px;
  }
  .set-group .side-label {
    display: block;
  }
  .set-group ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .set-group a {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 7px 12px;
    border-radius: var(--r-btn);
    color: var(--ink-2);
    font-weight: 600;
    transition: background-color 0.12s ease-out;
  }
  .set-group a span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .set-group a:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .set-group a[aria-current="page"] {
    background: var(--surface-2);
    color: var(--ink);
    font-weight: 700;
  }
  .set-group a[aria-current="page"] :global(svg) {
    color: var(--forest);
  }
  .set-empty {
    margin: 0;
    padding: 0 12px;
    font-size: 13px;
    color: var(--ink-2);
  }
  /* One column for the whole window: the sidebar sits above the page, items two to a row so it stays short. */
  @media (max-width: 720px) {
    .set-side {
      flex-direction: column;
      align-items: stretch;
      overflow: visible;
    }
    .set-nav {
      overflow: visible;
    }
    .set-group ul {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .set-group a {
      min-height: 44px;
    }
  }
</style>
