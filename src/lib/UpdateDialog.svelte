<script lang="ts">
  import Dialog from "./Dialog.svelte";
  import { plural, t } from "./i18n.svelte";
  import { install, liveSessions, update } from "./update.svelte";

  const version = $derived(update.view?.available ?? "");
</script>

<Dialog bind:open={() => update.confirming, (v) => (update.confirming = v)} labelledby="update-title">
  <div class="d-body" id="update-dialog">
    <h2 id="update-title">{t("shell.update.confirmTitle")}</h2>
    <p class="meta" style="margin:0;font-size:14px">{plural(liveSessions(), "shell.update.confirmOne", "shell.update.confirmMany", { version })}</p>
    <div class="d-foot">
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (update.confirming = false)}>{t("shell.update.keep")}</button>
      <button class="btn danger" type="button" id="btn-update-stop-sessions" onclick={install}>{t("shell.update.stopAndInstall")}</button>
    </div>
  </div>
</Dialog>
