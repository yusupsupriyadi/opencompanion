<script lang="ts">
  import { toast } from "./store.svelte";
</script>

<div class="toast" class:show={toast.show} role="status" aria-live="polite">{toast.text}</div>
