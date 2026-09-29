<script lang="ts">
  import Check from "phosphor-svelte/lib/Check";
  import { tick, untrack } from "svelte";
  import { api, errorText, type Automation, type AutomationDraft, type CliKind, type Mode, type PermMode } from "./api";
  import ScheduleField from "./ScheduleField.svelte";
  import SessionFields, { checkSession } from "./SessionFields.svelte";
  import { t, tb } from "./i18n.svelte";
  import { app } from "./store.svelte";

  // Creates an automation, or changes `initial`. The session fields are New session's own.
  let {
    initial = null,
    onsubmit,
    oncancel,
  }: {
    initial?: Automation | null;
    onsubmit: (draft: AutomationDraft) => Promise<void>;
    oncancel: () => void;
  } = $props();

  const ID = "af";
  const firstInstalled = (): CliKind => app.clis.find((c) => c.path && c.kind !== "gemini")?.kind ?? "claude";
  const start = untrack(() => initial);

  let name = $state(start?.name ?? "");
  let cli = $state<CliKind>(start?.cli ?? firstInstalled());
  let cwd = $state(start?.cwd ?? "");
  let mode = $state<Mode>(start?.mode ?? "headless");
  let prompt = $state(start?.prompt ?? "");
  let permissionMode = $state<PermMode>(start?.permissionMode ?? app.settings?.permissionMode ?? "ask");
  let schedule = $state(start?.schedule ?? "");
  let nameError = $state("");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);
  let promptEl: HTMLTextAreaElement | undefined = $state();

  const installed = $derived(app.clis.filter((c) => c.path));

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    if (!name.trim()) {
      nameError = t("auto.form.nameNeeded");
      document.getElementById(`${ID}-name`)?.focus();
      return;
    }
    const values = { cli, cwd: cwd.trim(), mode, prompt: prompt.trim(), permissionMode };
    const problems = await checkSession(values);
    if (problems.folderError || problems.promptError || problems.failure) {
      folderError = problems.folderError ?? "";
      promptError = problems.promptError ?? "";
      failure = problems.failure ?? "";
      await tick();
      if (folderError) document.getElementById(`${ID}-folder`)?.focus();
      else if (promptError) promptEl?.focus();
      return;
    }
    if (!schedule) {
      // Some days with no day picked; the field says so under the days.
      document.getElementById(`${ID}-day-1`)?.focus();
      return;
    }
    busy = true;
    try {
      await onsubmit({ name: name.trim(), ...values, schedule, enabled: start?.enabled ?? true });
    } catch (err) {
      const msg = errorText(err);
      if (msg === "This folder does not exist.") folderError = msg;
      else failure = msg;
    } finally {
      busy = false;
    }
  }
</script>

<form class="card auto-form" id="automation-form" novalidate onsubmit={submit} aria-label={start ? t("auto.form.editTitle") : t("auto.form.newTitle")}>
  <div class="field">
    <label class="label" for="{ID}-name">{t("auto.form.name")}</label>
    <input
      class="input"
      id="{ID}-name"
      bind:value={name}
      oninput={() => (nameError = "")}
      placeholder={t("auto.form.namePlaceholder")}
      autocomplete="off"
      maxlength="80"
      aria-invalid={nameError ? "true" : undefined}
      aria-describedby={nameError ? `${ID}-name-error` : undefined}
    />
    {#if nameError}<p class="error" id="{ID}-name-error">{nameError}</p>{/if}
  </div>

  <SessionFields bind:cli bind:cwd bind:mode bind:prompt bind:permissionMode bind:folderError bind:promptError bind:promptEl idPrefix={ID} />

  <ScheduleField bind:value={schedule} idPrefix={ID} preview={api.previewSchedule} />

  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  <div class="foot">
    <button class="btn secondary" type="button" onclick={oncancel}>{t("auto.form.cancel")}</button>
    <button class="btn primary" type="submit" id="btn-save-automation" disabled={busy || installed.length === 0}>
      <Check size={16} aria-hidden="true" /><span>{busy ? t("auto.form.saving") : start ? t("auto.form.save") : t("auto.form.create")}</span>
    </button>
  </div>
</form>

<style>
  .auto-form {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
    min-width: 0;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }
</style>
