<script lang="ts">
  import type { Snippet } from "svelte";
  import { refocus } from "./focus";

  // Native <dialog>: Esc closes it and focus is trapped by the browser. Focus returns to
  // whatever opened it.
  let {
    open = $bindable(false),
    labelledby,
    wide = false,
    children,
  }: { open?: boolean; labelledby: string; wide?: boolean; children: Snippet } = $props();

  let el: HTMLDialogElement | undefined = $state();
  let opener: Element | null = null;

  $effect(() => {
    if (!el) return;
    if (open && !el.open) {
      opener = document.activeElement;
      el.showModal();
    } else if (!open && el.open) {
      el.close();
    }
  });

  function onclose() {
    open = false;
    // The opener can be gone, such as a sidebar row whose session was just deleted.
    if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
    else refocus();
  }
</script>

<dialog bind:this={el} aria-labelledby={labelledby} {onclose} class:wide>
  {#if open}{@render children()}{/if}
</dialog>

<style>
  dialog.wide {
    width: min(720px, calc(100vw - 32px));
  }
</style>
