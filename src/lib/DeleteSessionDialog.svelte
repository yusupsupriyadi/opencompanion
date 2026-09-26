<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Trash from "phosphor-svelte/lib/Trash";
  import { errorText } from "./api";
  import Dialog from "./Dialog.svelte";
  import { CLI_LABEL, folderName } from "./format";
  import { t } from "./i18n.svelte";
  import { deleteSession, pendingDelete, showToast } from "./store.svelte";

  let busy = $state(false);
  let error = $state("");

  function close() {
    pendingDelete.session = null;
    error = "";
  }

  async function confirm() {
    const s = pendingDelete.session;
    if (!s || busy) return;
    busy = true;
    error = "";
    try {
      await deleteSession(s.id);
      close();
      showToast(t("shell.delete.done", { title: s.title }));
      if (page.url.pathname === "/session" && page.url.searchParams.get("id") === s.id) await goto("/");
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog bind:open={() => pendingDelete.session !== null, (v) => !v && close()} labelledby="del-title">
  {#if pendingDelete.session}
    {@const s = pendingDelete.session}
    <div class="d-body" id="delete-session-dialog">
      <h2 id="del-title">{t("shell.delete.title")}</h2>
      <p class="meta" style="margin:0;font-size:14px">
        {t("shell.delete.body", { cli: CLI_LABEL[s.cli], folder: folderName(s.cwd), title: s.title })}
      </p>
      {#if error}<p class="err-text" role="alert" style="margin:0">{error}</p>{/if}
      <div class="d-foot">
        <span class="grow"></span>
        <button class="btn secondary" type="button" onclick={close}>{t("shell.delete.keep")}</button>
        <button class="btn danger" type="button" id="btn-delete-session" disabled={busy} onclick={confirm}>
          <Trash size={16} aria-hidden="true" />{busy ? t("shell.delete.deleting") : t("shell.deleteSession")}
        </button>
      </div>
    </div>
  {/if}
</Dialog>
