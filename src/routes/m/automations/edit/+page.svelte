<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import Check from "phosphor-svelte/lib/Check";
  import Play from "phosphor-svelte/lib/Play";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import Trash from "phosphor-svelte/lib/Trash";
  import { onMount, tick } from "svelte";
  import type { Automation, AutomationDetail, AutomationDraft, AutomationRun, CliKind, Mode, PermMode } from "$lib/api";
  import Dialog from "$lib/Dialog.svelte";
  import { duration } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import OutcomeChip from "$lib/OutcomeChip.svelte";
  import { call, notify, onMessage, type PhoneOptions } from "$lib/phone.svelte";
  import PhoneSessionFields, { OTHER_FOLDER } from "$lib/PhoneSessionFields.svelte";
  import ScheduleField from "$lib/ScheduleField.svelte";
  import { whenText } from "$lib/schedule";

  // `?new` makes one, `?id=` changes one. The form fills once; later changes from elsewhere
  // refresh the runs and the header only, so a half-edited form is never overwritten.
  const ID = "pa";
  const id = page.url.searchParams.get("id");

  let options = $state<PhoneOptions | null>(null);
  let detail = $state<AutomationDetail | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");

  let name = $state("");
  let cli = $state<CliKind>("claude");
  let folder = $state("");
  let typed = $state("");
  let mode = $state<Mode>("headless");
  let prompt = $state("");
  let permissionMode = $state<PermMode>("ask");
  let schedule = $state("");
  let nameError = $state("");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);
  let running = $state(false);
  let deleteOpen = $state(false);
  let deleteBusy = $state(false);
  let deleteError = $state("");

  const cwd = $derived((folder === OTHER_FOLDER ? typed : folder).trim());
  const installed = $derived(options?.clis.filter((c) => c.path) ?? []);
  const a = $derived(detail?.automation ?? null);

  function fill(o: PhoneOptions, from: Automation | null) {
    const usable = o.clis.filter((c) => c.path);
    if (!from) {
      cli = usable.find((c) => c.kind !== "gemini")?.kind ?? usable[0]?.kind ?? "claude";
      folder = o.folders[0]?.path || OTHER_FOLDER;
      permissionMode = o.permissionMode;
      return;
    }
    name = from.name;
    cli = from.cli;
    if (o.folders.some((f) => f.path === from.cwd)) folder = from.cwd;
    else [folder, typed] = [OTHER_FOLDER, from.cwd];
    mode = from.mode;
    prompt = from.prompt;
    permissionMode = from.permissionMode;
    schedule = from.schedule;
  }

  async function load() {
    loadState = "loading";
    try {
      const [o, d] = await Promise.all([
        call<PhoneOptions>("/api/options"),
        id ? call<AutomationDetail>(`/api/automations/${id}`) : Promise.resolve(null),
      ]);
      options = o;
      detail = d;
      fill(o, d?.automation ?? null);
      loadState = "ready";
    } catch (e) {
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  async function reloadDetail() {
    if (!id) return;
    try {
      detail = await call<AutomationDetail>(`/api/automations/${id}`);
    } catch {
      // Deleted elsewhere: the next save or run says so.
    }
  }

  onMount(() => {
    load();
    const off = onMessage((m) => {
      if (m.type === "automations") reloadDetail();
    });
    return off;
  });

  const preview = (s: string) => call<{ times: number[] }>("/api/automations/preview", { method: "POST", body: JSON.stringify({ schedule: s }) }).then((r) => r.times);

  async function focus(el: string) {
    await tick();
    document.getElementById(el)?.focus();
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    if (!name.trim()) {
      nameError = t("auto.form.nameNeeded");
      focus(`${ID}-name`);
      return;
    }
    if (!cwd) {
      folderError = t("phone.new.folderRequired");
      focus(folder === OTHER_FOLDER ? `${ID}-path` : `${ID}-folder`);
      return;
    }
    if (mode === "headless" && !prompt.trim()) {
      promptError = t("phone.new.promptRequired");
      focus(`${ID}-prompt`);
      return;
    }
    if (mode === "headless" && cli === "gemini") {
      failure = t("phone.new.geminiHeadless");
      return;
    }
    if (!schedule) {
      focus(`${ID}-day-1`);
      return;
    }
    const draft: AutomationDraft = { name: name.trim(), cli, cwd, mode, prompt: prompt.trim(), permissionMode, schedule, enabled: a?.enabled ?? true };
    busy = true;
    try {
      const r = id
        ? await call<{ automation: Automation }>(`/api/automations/${id}`, { method: "PUT", body: JSON.stringify(draft) })
        : await call<{ automation: Automation }>("/api/automations", { method: "POST", body: JSON.stringify(draft) });
      const saved = r.automation;
      notify(
        saved.enabled && saved.nextRunAt
          ? t("auto.toast.saved", { name: saved.name, when: whenText(saved.nextRunAt) })
          : t("auto.toast.savedPaused", { name: saved.name }),
      );
      if (id) await reloadDetail();
      else await goto("/m/automations", { replaceState: true });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg === "This folder does not exist.") {
        folderError = msg;
        focus(folder === OTHER_FOLDER ? `${ID}-path` : `${ID}-folder`);
      } else {
        failure = msg;
      }
    } finally {
      busy = false;
    }
  }

  async function runNow() {
    if (!a || running) return;
    running = true;
    try {
      const { run } = await call<{ run: AutomationRun }>(`/api/automations/${a.id}/run`, { method: "POST" });
      if (run.outcome === "skipped") notify(t("auto.toast.skipped", { name: a.name }));
      else if (run.outcome === "failed") notify(t("auto.toast.failed", { name: a.name, error: tb(run.error) }));
      else notify(t("auto.toast.started", { name: a.name }));
    } catch (e) {
      notify(tb(e instanceof Error ? e.message : String(e)));
    } finally {
      running = false;
    }
    reloadDetail();
  }

  async function confirmDelete() {
    if (!a || deleteBusy) return;
    deleteBusy = true;
    deleteError = "";
    try {
      await call(`/api/automations/${a.id}`, { method: "DELETE" });
      deleteOpen = false;
      notify(t("auto.toast.deleted", { name: a.name }));
      await goto("/m/automations", { replaceState: true });
    } catch (e) {
      deleteError = tb(e instanceof Error ? e.message : String(e));
    } finally {
      deleteBusy = false;
    }
  }
</script>

<svelte:head><title>{a?.name ?? t("auto.form.newTitle")} · OpenCompanion</title></svelte:head>

<header class="bar m-bar-tight">
  <a class="m-icon-btn m-back" href="/m/automations" aria-label={t("auto.m.back")} title={t("auto.m.back")}>
    <CaretLeft size={20} aria-hidden="true" />
  </a>
  <h1 class="m-bar-title">{a?.name ?? (id ? t("auto.form.editTitle") : t("auto.form.newTitle"))}</h1>
</header>
<main class="content dense" id="phone-automation">
  {#if loadState === "loading"}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("auto.m.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{tb(loadError)}</p>
    {#if id}
      <a class="btn secondary block" href="/m/automations">{t("auto.m.back")}</a>
    {:else}
      <button class="btn secondary block" type="button" onclick={load}>{t("phone.tryAgain")}</button>
    {/if}
  {:else if options}
    {#if a}
      <div class="m-auto-acts">
        <button class="btn secondary" type="button" id="btn-run-automation" disabled={running} onclick={runNow}>
          {#if running}<SpinnerGap size={16} class="spin" aria-hidden="true" />{:else}<Play size={16} aria-hidden="true" />{/if}{t("auto.runNow")}
        </button>
        <button class="btn ghost m-danger" type="button" id="btn-delete-automation" onclick={() => ((deleteError = ""), (deleteOpen = true))}>
          <Trash size={16} aria-hidden="true" />{t("auto.delete")}
        </button>
      </div>
      <section class="m-sec" aria-labelledby="m-runs">
        <h2 id="m-runs">{t("auto.runs")}</h2>
        {#if detail?.runs.length}
          <ul class="m-runs">
            {#each detail.runs.slice(0, 10) as r (r.id)}
              <li>
                <span class="m-run-top"><OutcomeChip outcome={r.outcome} /><b>{whenText(r.dueAt)}</b></span>
                {#if r.outcome === "late"}<span class="meta">{t("auto.outcomeHelp.late", { late: duration(r.ranAt - r.dueAt) })}</span>{/if}
                {#if r.outcome === "skipped"}<span class="meta">{t("auto.outcomeHelp.skipped")}</span>{/if}
                {#if r.error}<span class="err-text">{tb(r.error)}</span>{/if}
                {#if r.sessionId}<a class="btn secondary sm" href="/m/session?id={r.sessionId}">{t("auto.openSession")}</a>
                {:else if r.outcome === "started" || r.outcome === "late"}<span class="meta">{t("auto.sessionGone")}</span>{/if}
              </li>
            {/each}
          </ul>
        {:else}
          <p class="m-p">{a.enabled && a.nextRunAt ? t("auto.runsEmpty", { when: whenText(a.nextRunAt) }) : t("auto.runsEmptyPaused")}</p>
        {/if}
      </section>
    {/if}

    <form class="m-form" novalidate onsubmit={submit}>
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

      <PhoneSessionFields {options} bind:cli bind:folder bind:typed bind:mode bind:prompt bind:permissionMode bind:folderError bind:promptError idPrefix={ID} />

      <ScheduleField bind:value={schedule} idPrefix={ID} {preview} />

      {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

      <button class="btn primary block" type="submit" id="btn-save-automation" disabled={busy || installed.length === 0}>
        {#if busy}<SpinnerGap size={16} class="spin" aria-hidden="true" />{:else}<Check size={16} aria-hidden="true" />{/if}
        {busy ? t("auto.form.saving") : id ? t("auto.form.save") : t("auto.form.create")}
      </button>
    </form>
  {/if}
</main>

<Dialog bind:open={deleteOpen} labelledby="auto-del-sheet-title" sheet>
  {#if a}
    <div class="d-body" style="gap:14px">
      <h2 id="auto-del-sheet-title" style="font-size:20px">{t("auto.deleteTitle")}</h2>
      <p class="m-p">{t("auto.deleteBody", { name: a.name })}</p>
      {#if deleteError}<p class="err-text" role="alert" style="margin:0">{deleteError}</p>{/if}
      <div class="d-foot">
        <button class="btn danger" type="button" disabled={deleteBusy} onclick={confirmDelete}><Trash size={16} aria-hidden="true" />{t("auto.delete")}</button>
        <button class="btn secondary" type="button" onclick={() => (deleteOpen = false)}>{t("auto.keep")}</button>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .m-back {
    margin-left: -8px;
  }
  .m-bar-title {
    flex: 1;
    min-width: 0;
    font-size: 17px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-auto-acts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .m-auto-acts .btn {
    min-height: 44px;
    white-space: normal;
  }
  .m-danger {
    color: var(--st-err);
  }
  .m-runs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .m-runs li {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .m-run-top {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .m-runs .btn {
    min-height: 40px;
  }
</style>
