<script lang="ts">
  import type { Status } from "./api";
  import { STATUS } from "./format";

  let { status }: { status: Status } = $props();
  const s = $derived(STATUS[status]);
</script>

<span class="chip {s.chip}">{s.label}</span>
