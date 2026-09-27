<script lang="ts">
  import type { CliKind } from "./api";
  import claudeUrl from "./logos/claude.svg?url";
  import geminiUrl from "./logos/gemini.svg?url";
  import openaiRaw from "./logos/openai.svg?raw";

  let { kind, small = false, bare = false }: { kind: CliKind; small?: boolean; bare?: boolean } = $props();

  // The OpenAI mark ships without a fill; currentColor keeps it visible in both themes.
  const openai = openaiRaw
    .replace(/\s(width|height)="[^"]*"/g, "")
    .replace("<path ", '<path fill="currentColor" ');
</script>

<span class="climark" class:sm={small} class:bare class:has-logo={kind !== "opencode" && kind !== "ccs" && kind !== "pi" && kind !== "omp"} aria-hidden="true">
  {#if kind === "claude"}
    <img src={claudeUrl} alt="" />
  {:else if kind === "gemini"}
    <img src={geminiUrl} alt="" />
  {:else if kind === "codex"}
    {@html openai}
  {:else if kind === "ccs"}
    CCS
  {:else if kind === "pi"}
    Pi
  {:else if kind === "omp"}
    omp
  {:else}
    OC
  {/if}
</span>
