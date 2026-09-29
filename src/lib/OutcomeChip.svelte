<script lang="ts">
  import type { RunOutcome } from "./api";
  import { t } from "./i18n.svelte";

  // The status chip colors (DESIGN.md section 2): a start reads like Running, a late one like
  // waiting for you, a skip like done, a failure like an error.
  const CHIP: Record<RunOutcome, string> = { started: "run", late: "wait", skipped: "idle", failed: "err" };

  let { outcome }: { outcome: RunOutcome } = $props();
</script>

<span class="chip {CHIP[outcome]}">{t(`auto.outcome.${outcome}`)}</span>
