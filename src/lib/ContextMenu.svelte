<script lang="ts">
  import { closeMenu, menu, type MenuItem } from "./context-menu.svelte";

  const EDGE = 8;

  // Runs once per opening ({#key} below): keeps the menu inside the window, clear of the title
  // bar (a window drag region takes clicks over it), moves focus in, and closes when the page
  // moves under it or the pointer goes elsewhere.
  function place(node: HTMLElement) {
    const { width, height } = node.getBoundingClientRect();
    const top = (parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--titlebar")) || 0) + EDGE / 2;
    const y = menu.y + height <= innerHeight - EDGE ? menu.y : menu.y - height;
    node.style.left = `${Math.max(EDGE, Math.min(menu.x, innerWidth - width - EDGE))}px`;
    node.style.top = `${Math.max(top, y)}px`;
    node.querySelector<HTMLElement>('[role="menuitem"]')?.focus({ preventScroll: true });

    const shut = () => closeMenu(false);
    const away = (e: PointerEvent) => {
      if (!node.contains(e.target as Node)) shut();
    };
    document.addEventListener("pointerdown", away, true);
    addEventListener("scroll", shut, { capture: true, passive: true });
    addEventListener("resize", shut);
    addEventListener("blur", shut);
    return () => {
      document.removeEventListener("pointerdown", away, true);
      removeEventListener("scroll", shut, { capture: true });
      removeEventListener("resize", shut);
      removeEventListener("blur", shut);
    };
  }

  function onkeydown(e: KeyboardEvent) {
    const items = [...(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const go = (i: number) => items[(i + items.length) % items.length]?.focus();
    if (e.key === "ArrowDown") go(at + 1);
    else if (e.key === "ArrowUp") go(at <= 0 ? -1 : at - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(-1);
    else if (e.key === "Escape" || e.key === "Tab") closeMenu();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function choose(item: MenuItem) {
    if (item.disabled) return;
    // Focus goes back first, so a dialog the action opens returns there when it closes.
    closeMenu();
    item.action();
  }
</script>

{#if menu.open}
  {#key menu.seq}
    <div class="ctx-menu" id="context-menu" role="menu" tabindex="-1" aria-label={menu.label} {onkeydown} oncontextmenu={(e) => e.preventDefault()} {@attach place}>
      {#each menu.items as item, i (i)}
        {#if item}
          <button
            type="button"
            role="menuitem"
            tabindex="-1"
            class:danger={item.danger}
            aria-disabled={item.disabled || undefined}
            onpointermove={(e) => {
              if (document.activeElement !== e.currentTarget) e.currentTarget.focus({ preventScroll: true });
            }}
            onclick={() => choose(item)}
          >
            {#if item.icon}<item.icon size={16} aria-hidden="true" />{/if}
            <span class="label">{item.label}</span>
            {#if item.hint}<span class="hint">{item.hint}</span>{/if}
          </button>
        {:else}
          <div class="sep" role="separator"></div>
        {/if}
      {/each}
    </div>
  {/key}
{/if}

<style>
  .ctx-menu {
    position: fixed;
    z-index: 45;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 210px;
    max-width: 320px;
    padding: 6px;
    border-radius: var(--r-md);
    border: 1px solid var(--line-strong);
    background: var(--surface);
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 6px 10px;
    border: 0;
    border-radius: var(--r-btn);
    background: transparent;
    color: var(--ink);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }
  button :global(svg) {
    flex-shrink: 0;
    color: var(--ink-2);
  }
  /* Opening focuses the first item and the pointer moves focus, so one item is lit whether the
     mouse or the keys chose it. */
  button:focus {
    background: var(--surface-2);
  }
  /* Delete reads red at rest and lit; the menu's tint (--glass-menu) is what keeps it at 4.5:1. A
     refused Delete stays grey. */
  .danger:not([aria-disabled]),
  .danger:not([aria-disabled]) :global(svg) {
    color: var(--st-err);
  }
  button[aria-disabled] {
    cursor: default;
    color: var(--ink-2);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    font-size: 12px;
    font-weight: 400;
    white-space: nowrap;
  }
  .sep {
    height: 1px;
    margin: 3px 6px;
    background: var(--line);
  }
  @media (pointer: coarse) {
    button {
      min-height: 44px;
    }
  }
</style>
