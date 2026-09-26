<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Browsers from "phosphor-svelte/lib/Browsers";
  import Minus from "phosphor-svelte/lib/Minus";
  import Square from "phosphor-svelte/lib/Square";
  import X from "phosphor-svelte/lib/X";
  import { onMount } from "svelte";
  import { t } from "./i18n.svelte";

  // The native frame is off (`decorations: false`), so this bar is the drag handle and the window buttons.
  // Double-clicking the bar maximizes: Tauri's drag-region script handles that.
  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    const sync = async () => {
      maximized = await win.isMaximized();
    };
    sync();
    const unlisten = win.onResized(sync);
    return () => {
      unlisten.then((off) => off());
    };
  });
</script>

<div class="titlebar" id="titlebar" data-tauri-drag-region>
  <div class="tb-controls">
    <button
      type="button"
      class="tb-btn"
      id="btn-window-minimize"
      aria-label={t("shell.titlebar.minimize")}
      title={t("shell.titlebar.minimize")}
      onclick={() => win.minimize()}
    >
      <span><Minus size={16} aria-hidden="true" /></span>
    </button>
    <button
      type="button"
      class="tb-btn"
      id="btn-window-maximize"
      aria-label={maximized ? t("shell.titlebar.restore") : t("shell.titlebar.maximize")}
      title={maximized ? t("shell.titlebar.restore") : t("shell.titlebar.maximize")}
      onclick={() => win.toggleMaximize()}
    >
      <span>
        {#if maximized}<Browsers size={16} aria-hidden="true" />{:else}<Square size={14} aria-hidden="true" />{/if}
      </span>
    </button>
    <button type="button" class="tb-btn close" id="btn-window-close" aria-label={t("shell.close")} title={t("shell.close")} onclick={() => win.close()}>
      <span><X size={16} aria-hidden="true" /></span>
    </button>
  </div>
</div>
