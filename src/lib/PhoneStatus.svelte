<script lang="ts">
  import type { Status } from "./api";
  import { STATUS } from "./format";
  import { t } from "./i18n.svelte";

  // The phone's status chip: the desktop chip with a dot that moves only while its state is live,
  // breathing while the CLI works and ringing while it waits for you (phone.css, .m-dot).
  let { status }: { status: Status } = $props();
  const s = $derived(STATUS[status]);
  const motion = $derived(status === "waiting" ? "ask" : status === "running" || status === "starting" ? "live" : "");
  // "Waiting for you" does not fit beside a title on a phone; the Needs you panel says who waits.
  const label = $derived(status === "waiting" ? t("phone.status.waiting") : s.label);
</script>

<span class="chip {s.chip} m-status"><i class="m-dot {motion}" aria-hidden="true"></i>{label}</span>
