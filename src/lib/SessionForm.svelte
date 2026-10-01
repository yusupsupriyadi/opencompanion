<script lang="ts" module>
  export type { SessionValues } from "./SessionFields.svelte";
</script>

<script lang="ts">
  import Play from "phosphor-svelte/lib/Play";
  import TerminalIcon from "phosphor-svelte/lib/Terminal";
  import X from "phosphor-svelte/lib/X";
  import { tick, untrack } from "svelte";
  import { errorText, type CliKind, type Mode, type PermMode as Perm } from "./api";
  import SessionFields, { checkSession, type SessionValues } from "./SessionFields.svelte";
  import { CLI_LABEL, folderName } from "./format";
  import { t, tb } from "./i18n.svelte";
  import { app } from "./store.svelte";

  let {
    title,
    initial,
    submitText,
    onsubmit,
    oncancel,
    idPrefix = "ns",
    blank = false,
  }: {
    title: string;
    initial: Partial<SessionValues>;
    submitText?: (v: SessionValues) => string;
    onsubmit: (v: SessionValues) => Promise<void>;
    oncancel: () => void;
    idPrefix?: string;
    /** Offers a blank terminal beside the CLIs (New session). */
    blank?: boolean;
  } = $props();

  const installed = $derived(app.clis.filter((c) => c.path));
  // With no CLI installed at all, a blank terminal is the one choice that can start.
  const firstInstalled = (): CliKind =>
    app.clis.find((c) => c.path && c.kind !== "gemini")?.kind ?? (blank && app.clisState === "ready" ? "terminal" : "claude");

  // The form starts from `initial` once; the dialog remounts it every time it opens.
  const start = untrack(() => ({ ...initial }));
  let cli = $state<CliKind>(start.cli ?? firstInstalled());
  let cwd = $state(start.cwd ?? "");
  let mode = $state<Mode>(start.mode ?? "interactive");
  let prompt = $state(start.prompt ?? "");
  let permissionMode = $state<Perm>(start.permissionMode ?? app.settings?.permissionMode ?? "ask");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);
  let promptEl: HTMLTextAreaElement | undefined = $state();

  // A blank terminal hides the mode and the first message, so whatever they held before it was picked is not sent.
  const shellOnly = $derived(cli === "terminal");
  const values = $derived({
    cli,
    cwd: cwd.trim(),
    mode: shellOnly ? "interactive" : mode,
    prompt: shellOnly ? "" : prompt.trim(),
    permissionMode,
  });
  const label = $derived.by(() => {
    if (submitText) return submitText(values);
    const folder = values.cwd ? folderName(values.cwd) : "…";
    return shellOnly ? t("shell.form.openTerminal", { folder }) : t("shell.form.start", { cli: CLI_LABEL[cli], folder });
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    const problems = await checkSession(values);
    if (problems.folderError || problems.promptError || problems.failure) {
      folderError = problems.folderError ?? "";
      promptError = problems.promptError ?? "";
      failure = problems.failure ?? "";
      await tick();
      if (folderError) document.getElementById(`${idPrefix}-folder`)?.focus();
      else if (promptError) promptEl?.focus();
      return;
    }
    busy = true;
    try {
      await onsubmit(values);
    } catch (err) {
      failure = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="d-body" novalidate onsubmit={submit}>
  <div class="row">
    <h2 id="{idPrefix}-title" class="grow">{title}</h2>
    <button class="icon-btn" type="button" aria-label={t("shell.close")} onclick={oncancel}><X size={18} aria-hidden="true" /></button>
  </div>

  <SessionFields bind:cli bind:cwd bind:mode bind:prompt bind:permissionMode bind:folderError bind:promptError bind:promptEl {idPrefix} {blank} />

  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  <div class="d-foot">
    <span class="meta grow">{t("shell.form.escHint")}</span>
    <button class="btn secondary" type="button" onclick={oncancel}>{t("shell.form.cancel")}</button>
    <button class="btn primary" type="submit" disabled={busy || (installed.length === 0 && !shellOnly)}>
      {#if shellOnly}<TerminalIcon size={16} aria-hidden="true" />{:else}<Play size={16} aria-hidden="true" />{/if}<span>{busy ? t("shell.form.starting") : label}</span>
    </button>
  </div>
</form>
