<script lang="ts">
  import ArrowDown from "phosphor-svelte/lib/ArrowDown";
  import { untrack, type Snippet } from "svelte";
  import { t } from "./i18n.svelte";

  // A pane that follows the newest output, as a terminal does, until the reader scrolls back.
  // Then it stays where they are, and a button jumps to the end, marked once new output arrives.
  let { label, version, children }: { label: string; version: unknown; children: Snippet } = $props();

  let box: HTMLDivElement | undefined = $state();
  let atEnd = $state(true);
  let unseen = $state(false);
  // Set while a jump scrolls smoothly, so its own scroll events do not read as scrolling back.
  let jumping = false;

  const nearEnd = (el: HTMLElement) => el.scrollHeight - el.scrollTop - el.clientHeight <= 24;

  function onscroll() {
    if (!box) return;
    const end = nearEnd(box);
    if (jumping) {
      if (!end) return;
      jumping = false;
    }
    atEnd = end;
    if (end) unseen = false;
  }

  // A finger or wheel during a smooth jump takes the pane back.
  function takeOver() {
    jumping = false;
  }

  $effect(() => {
    void version;
    untrack(() => {
      if (!box) return;
      if (atEnd) box.scrollTop = box.scrollHeight;
      else unseen = true;
    });
  });

  function jump() {
    if (!box) return;
    atEnd = true;
    unseen = false;
    const still = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    if (!still && typeof box.scrollTo === "function") {
      jumping = true;
      box.scrollTo({ top: box.scrollHeight, behavior: "smooth" });
    } else {
      box.scrollTop = box.scrollHeight;
    }
  }

  const jumpLabel = $derived(unseen ? t("phone.session.jumpNew") : t("phone.session.jump"));
</script>

<div class="m-pane">
  <!-- It scrolls on its own, so a keyboard has to be able to reach it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="m-scroll" bind:this={box} {onscroll} ontouchstart={takeOver} onwheel={takeOver} role="region" aria-label={label} tabindex="0">
    {@render children()}
  </div>
  {#if !atEnd}
    <button class="m-icon-btn m-jump m-appear" type="button" aria-label={jumpLabel} title={jumpLabel} onclick={jump}>
      <ArrowDown size={18} weight="bold" aria-hidden="true" />
      {#if unseen}<i class="m-unseen" aria-hidden="true"></i>{/if}
    </button>
  {/if}
</div>
