<script lang="ts">
  import Cloud from "phosphor-svelte/lib/Cloud";
  import { t } from "./i18n.svelte";
  import { phone } from "./phone.svelte";

  // The top bar of the phone's tab screens: the wordmark and the connection to the desktop, whose
  // dot breathes while it connects. A desktop that went away gets the offline screen instead.
  const online = $derived(phone.connection === "online");
</script>

<header class="bar m-bar-tight">
  <span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span>
  <span class="m-conn" class:on={online} role="status">
    <i class="m-dot" class:live={!online} aria-hidden="true"></i>{online ? t("phone.conn.online") : t("phone.conn.connecting")}
  </span>
</header>
