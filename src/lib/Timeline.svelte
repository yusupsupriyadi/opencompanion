<script lang="ts">
  import type { EventRow } from "./api";
  import { clock } from "./format";

  let { events }: { events: EventRow[] } = $props();
  let box: HTMLPreElement | undefined = $state();

  type Line = { cls: string; text: string };

  function lines(row: EventRow): Line[] {
    const e = row.event;
    switch (e.kind) {
      case "started":
        return [{ cls: "t-d", text: `${clock(row.at)}  Session started` }];
      case "message":
        if (e.text.startsWith("You: ")) return [{ cls: "t-g", text: `› ${e.text.slice(5)}` }];
        if (/^(Approved|Denied|Stopped) /.test(e.text)) return [{ cls: e.text.startsWith("Approved") ? "t-g" : "t-r", text: `  ${e.text}` }];
        return [{ cls: "", text: e.text }];
      case "tool_call":
        return [{ cls: "t-d", text: `• ${e.tool} ${e.summary}`.trimEnd() }];
      case "file_changed":
        return [{ cls: "t-d", text: `  edited ${e.path}` }];
      case "permission_request":
        return [{ cls: "t-y", text: `? ${e.tool} asks for permission: ${e.summary}` }];
      case "permission_denied":
        return [{ cls: "t-r", text: `✗ ${e.tool} was refused: ${e.summary}` }];
      case "tool_failed":
        return [{ cls: "t-r", text: `✗ ${e.tool || "Tool"} failed: ${e.message}` }];
      case "retrying":
        return [{ cls: "t-y", text: e.message }];
      case "error":
        return [{ cls: "t-r", text: e.message }];
      case "done":
        return [{ cls: e.ok ? "t-g" : "t-r", text: e.ok ? `✓ Finished at ${clock(row.at)}` : `✗ Failed: ${e.summary}` }];
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

<pre class="term-out" bind:this={box} aria-live="polite">{#if all.length === 0}<span class="t-d">Waiting for the first event…</span>{/if}{#each all as l, i (i)}{#if i > 0}{"\n"}{/if}<span class={l.cls}>{l.text}</span>{/each}</pre>
