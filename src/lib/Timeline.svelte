<script lang="ts">
  import type { EventRow } from "./api";
  import { clock } from "./format";
  import { t } from "./i18n.svelte";

  let { events }: { events: EventRow[] } = $props();
  let box: HTMLPreElement | undefined = $state();

  type Line = { cls: string; text: string };

  function lines(row: EventRow): Line[] {
    const e = row.event;
    switch (e.kind) {
      case "started":
        return [{ cls: "t-d", text: `${clock(row.at)}  ${t("sessions.timeline.started")}` }];
      case "message":
        // The backend writes these in English; the prefix picks the color, not the words shown.
        if (e.text.startsWith("You: ")) return [{ cls: "t-g", text: `› ${e.text.slice(5)}` }];
        if (/^(Approved|Denied|Stopped) /.test(e.text)) return [{ cls: e.text.startsWith("Approved") ? "t-g" : "t-r", text: `  ${e.text}` }];
        return [{ cls: "", text: e.text }];
      case "tool_call":
        return [{ cls: "t-d", text: `• ${e.tool} ${e.summary}`.trimEnd() }];
      case "file_changed":
        return [{ cls: "t-d", text: `  ${t("sessions.timeline.edited", { path: e.path })}` }];
      case "permission_request":
        return [{ cls: "t-y", text: `? ${t("sessions.timeline.asks", { tool: e.tool, summary: e.summary })}` }];
      case "permission_denied":
        return [{ cls: "t-r", text: `✗ ${t("sessions.timeline.refused", { tool: e.tool, summary: e.summary })}` }];
      case "tool_failed":
        return [{ cls: "t-r", text: `✗ ${t("sessions.timeline.toolFailed", { tool: e.tool || t("sessions.timeline.tool"), message: e.message })}` }];
      case "retrying":
        return [{ cls: "t-y", text: e.message }];
      case "error":
        return [{ cls: "t-r", text: e.message }];
      case "done":
        return [
          {
            cls: e.ok ? "t-g" : "t-r",
            text: e.ok ? `✓ ${t("sessions.timeline.finished", { time: clock(row.at) })}` : `✗ ${t("sessions.timeline.failed", { summary: e.summary })}`,
          },
        ];
      default:
        return [];
    }
  }

  const all = $derived(events.flatMap(lines));

  $effect(() => {
    void all.length;
    if (box) box.scrollTop = box.scrollHeight;
  });
</script>

<pre class="term-out" bind:this={box} aria-live="polite">{#if all.length === 0}<span class="t-d">{t("sessions.timeline.waiting")}</span>{/if}{#each all as l, i (i)}{#if i > 0}{"\n"}{/if}<span class={l.cls}>{l.text}</span>{/each}</pre>
