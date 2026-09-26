<script lang="ts">
  import { t } from "./i18n.svelte";

  // Activity track (DESIGN.md section 6): ground line, soil specks, one mark per real event.
  let {
    marks,
    start,
    end,
    live = false,
    full = false,
  }: { marks: [number, string][]; start: number; end: number; live?: boolean; full?: boolean } = $props();

  const SPECKS = [6, 23, 41, 58, 77, 96, 118, 137, 152, 171, 190, 207];

  const events = $derived.by(() => {
    const span = Math.max(1, end - start);
    return marks
      .filter(([at]) => at >= start && at <= end + 1000)
      .map(([at, kind]) => ({
        left: Math.min(97, Math.max(0, ((at - start) / span) * 100)),
        cls: kind === "permission_request" ? "w" : kind === "error" || kind === "tool_failed" || kind === "permission_denied" ? "e" : "",
      }));
  });

  const label = $derived(marks.length === 0 ? t("shell.horizon.empty") : t("shell.horizon.count", { n: marks.length }));
</script>

<span class="horizon" class:full role="img" aria-label={label}>
  {#each SPECKS as x, i (x)}
    <i class="sp" style:left="{x / 2.2}%" style:top="{i % 2 ? 14 : 15}px"></i>
  {/each}
  {#each events as ev, i (i)}
    <i class="ev {ev.cls}" style:left="{ev.left}%"></i>
  {/each}
  {#if live}<i class="now"></i>{/if}
</span>
