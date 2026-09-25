<script lang="ts">
  import { goto } from "$app/navigation";
  import { api } from "./api";
  import Dialog from "./Dialog.svelte";
  import { CLI_LABEL, folderName } from "./format";
  import SessionForm, { type SessionValues } from "./SessionForm.svelte";
  import { pendingNew, showToast } from "./store.svelte";

  async function start(v: SessionValues) {
    const s = await api.startSession({ ...v, source: "manual" });
    pendingNew.open = false;
    showToast(`Started ${CLI_LABEL[v.cli]} in ${folderName(v.cwd)}.`);
    await goto(`/session?id=${s.id}`);
  }
</script>

<Dialog bind:open={pendingNew.open} labelledby="ns-title">
  <SessionForm title="New session" initial={{ cwd: pendingNew.cwd || undefined }} onsubmit={start} oncancel={() => (pendingNew.open = false)} />
</Dialog>
