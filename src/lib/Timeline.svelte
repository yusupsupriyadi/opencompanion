<script lang="ts">
  import type { EventRow } from "./api";
  import { t } from "./i18n.svelte";
  import { timelineLines } from "./timeline";

  let { events }: { events: EventRow[] } = $props();
  let box: HTMLPreElement | undefined = $state();

  const all = $derived(events.flatMap(timelineLines));

  $effect(() => {
    void all.length;
    if (box) box.scrollTop = box.scrollHeight;
  });
</script>

<pre class="term-out" bind:this={box} aria-live="polite">{#if all.length === 0}<span class="t-d">{t("sessions.timeline.waiting")}</span>{/if}{#each all as l, i (i)}{#if i > 0}{"\n"}{/if}<span class={l.cls}>{l.text}</span>{/each}</pre>
