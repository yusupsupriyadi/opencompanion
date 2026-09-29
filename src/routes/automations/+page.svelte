<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { listen } from "@tauri-apps/api/event";
  import ArrowLeft from "phosphor-svelte/lib/ArrowLeft";
  import Play from "phosphor-svelte/lib/Play";
  import Plus from "phosphor-svelte/lib/Plus";
  import Trash from "phosphor-svelte/lib/Trash";
  import { onMount } from "svelte";
  import { api, errorText, type AppInfo, type Automation, type AutomationDetail, type AutomationDraft, type AutomationView } from "$lib/api";
  import AutomationForm from "$lib/AutomationForm.svelte";
  import AutomationRow from "$lib/AutomationRow.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import { duration } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import OutcomeChip from "$lib/OutcomeChip.svelte";
  import { describe, whenText } from "$lib/schedule";
  import { app, showToast } from "$lib/store.svelte";

  // One route for the list (`/automations`), a new one (`?new`) and one automation (`?id=`), as the
  // static build has no dynamic segments.
  type LoadState = "loading" | "ready" | "error";

  const id = $derived(page.url.searchParams.get("id"));
  const creating = $derived(!id && page.url.searchParams.has("new"));

  let list = $state<AutomationView[]>([]);
  let listState = $state<LoadState>("loading");
  let listError = $state("");
  let detail = $state<AutomationDetail | null>(null);
  let detailState = $state<LoadState>("loading");
  let detailError = $state("");
  let info = $state<AppInfo | null>(null);
  let running = $state("");
  let deleting = $state<Automation | null>(null);
  let deleteBusy = $state(false);
  let deleteError = $state("");
  let asked = 0;

  /** `quiet` keeps what is shown while it reloads, for changes made elsewhere. */
  async function loadList(quiet = false) {
    if (!quiet) listState = "loading";
    try {
      list = await api.listAutomations();
      listState = "ready";
    } catch (e) {
      listState = "error";
      listError = errorText(e);
    }
  }

  async function loadDetail(which: string, quiet = false) {
    const ticket = ++asked;
    if (!quiet) detailState = "loading";
    try {
      const got = await api.getAutomation(which);
      if (ticket !== asked) return;
      detail = got;
      detailState = "ready";
    } catch (e) {
      if (ticket !== asked) return;
      detailState = "error";
      detailError = errorText(e);
    }
  }

  function reload() {
    if (id) loadDetail(id, true);
    else if (!creating) loadList(true);
  }

  $effect(() => {
    if (id) loadDetail(id);
    else if (!creating) loadList();
  });

  onMount(() => {
    api
      .appInfo()
      .then((i) => (info = i))
      .catch(() => undefined);
    // Saved, deleted or run here, on the phone, or by the schedule itself.
    const un = listen("automations-changed", reload);
    return () => {
      un.then((f) => f());
    };
  });

  const loginOff = $derived(Boolean(info?.canStartAtLogin && app.settings && !app.settings.startAtLogin));

  function nextText(a: Automation): string {
    return a.enabled && a.nextRunAt ? t("auto.nextRun", { when: whenText(a.nextRunAt, app.now) }) : t("auto.paused");
  }

  async function runNow(a: Automation) {
    if (running) return;
    running = a.id;
    try {
      const run = await api.runAutomation(a.id);
      if (run.outcome === "skipped") showToast(t("auto.toast.skipped", { name: a.name }));
      else if (run.outcome === "failed") showToast(t("auto.toast.failed", { name: a.name, error: tb(run.error) }));
      else showToast(t("auto.toast.started", { name: a.name }));
    } catch (e) {
      showToast(tb(errorText(e)));
    } finally {
      running = "";
    }
    reload();
  }

  async function toggle(a: Automation) {
    try {
      const saved = await api.setAutomationEnabled(a.id, !a.enabled);
      showToast(
        saved.enabled && saved.nextRunAt
          ? t("auto.toast.resumed", { name: saved.name, when: whenText(saved.nextRunAt, app.now) })
          : t("auto.toast.paused", { name: saved.name }),
      );
    } catch (e) {
      showToast(tb(errorText(e)));
    }
    reload();
  }

  function savedToast(a: Automation) {
    showToast(
      a.enabled && a.nextRunAt ? t("auto.toast.saved", { name: a.name, when: whenText(a.nextRunAt, app.now) }) : t("auto.toast.savedPaused", { name: a.name }),
    );
  }

  async function create(draft: AutomationDraft) {
    const saved = await api.saveAutomation(null, draft);
    savedToast(saved);
    await goto(`/automations?id=${saved.id}`);
  }

  async function update(draft: AutomationDraft) {
    if (!id) return;
    const saved = await api.saveAutomation(id, draft);
    savedToast(saved);
    await loadDetail(saved.id, true);
  }

  function askDelete(a: Automation) {
    deleteError = "";
    deleting = a;
  }

  async function confirmDelete() {
    const a = deleting;
    if (!a || deleteBusy) return;
    deleteBusy = true;
    deleteError = "";
    try {
      await api.deleteAutomation(a.id);
      deleting = null;
      showToast(t("auto.toast.deleted", { name: a.name }));
      if (id === a.id) await goto("/automations");
      else loadList(true);
    } catch (e) {
      deleteError = tb(errorText(e));
    } finally {
      deleteBusy = false;
    }
  }
</script>

<svelte:head><title>{detail && id ? detail.automation.name : t("auto.title")} · OpenCompanion</title></svelte:head>

<main class="main" id="automations-main">
  {#if creating}
    <header class="page-head">
      <div class="grow">
        <a class="back" href="/automations"><ArrowLeft size={16} aria-hidden="true" />{t("auto.back")}</a>
        <h1>{t("auto.form.newTitle")}</h1>
        <p class="sub">{t("auto.intro")}</p>
      </div>
    </header>
    <div class="auto-wrap">
      <AutomationForm onsubmit={create} oncancel={() => goto("/automations")} />
    </div>
  {:else if id}
    {#if detailState === "loading" && !detail}
      <p class="hint" role="status">{t("auto.loading")}</p>
    {:else if detailState === "error"}
      <div class="state-box" role="alert">
        <h2>{tb(detailError)}</h2>
        <a class="btn secondary" href="/automations"><ArrowLeft size={16} aria-hidden="true" />{t("auto.back")}</a>
      </div>
    {:else if detail}
      {@const a = detail.automation}
      <header class="page-head" id="automation-head">
        <div class="grow">
          <a class="back" href="/automations"><ArrowLeft size={16} aria-hidden="true" />{t("auto.back")}</a>
          <h1>{a.name}</h1>
          <p class="sub">{describe(a.schedule)} · {nextText(a)}</p>
        </div>
        <span class="active">
          <span id="auto-active-label">{t("auto.active")}</span>
          <button class="switch" type="button" role="switch" aria-checked={a.enabled} aria-labelledby="auto-active-label" onclick={() => toggle(a)}></button>
        </span>
        <button class="btn secondary" type="button" id="btn-run-automation" disabled={running === a.id} onclick={() => runNow(a)}>
          <Play size={16} aria-hidden="true" />{t("auto.runNow")}
        </button>
        <button class="btn ghost danger-text" type="button" id="btn-delete-automation" onclick={() => askDelete(a)}>
          <Trash size={16} aria-hidden="true" />{t("auto.delete")}
        </button>
      </header>
      <div class="auto-detail">
        {#key a.id}
          <AutomationForm initial={a} onsubmit={update} oncancel={() => goto("/automations")} />
        {/key}
        <section class="detail-side runs" id="automation-runs" aria-labelledby="runs-title">
          <h2 id="runs-title">{t("auto.runs")}</h2>
          {#if detail.runs.length === 0}
            <p>{a.enabled && a.nextRunAt ? t("auto.runsEmpty", { when: whenText(a.nextRunAt, app.now) }) : t("auto.runsEmptyPaused")}</p>
          {:else}
            <ul class="run-list">
              {#each detail.runs as r (r.id)}
                <li class="run">
                  <div class="run-top">
                    <OutcomeChip outcome={r.outcome} />
                    <span class="run-when">{whenText(r.dueAt, app.now)}</span>
                  </div>
                  {#if r.outcome === "late"}<p>{t("auto.outcomeHelp.late", { late: duration(r.ranAt - r.dueAt) })}</p>{/if}
                  {#if r.outcome === "skipped"}<p>{t("auto.outcomeHelp.skipped")}</p>{/if}
                  {#if r.error}<p class="err-text">{tb(r.error)}</p>{/if}
                  {#if r.sessionId}
                    <a class="btn secondary sm" href="/session?id={r.sessionId}">{t("auto.openSession")}</a>
                  {:else if r.outcome === "started" || r.outcome === "late"}
                    <p>{t("auto.sessionGone")}</p>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      </div>
    {/if}
  {:else}
    <header class="page-head">
      <div class="grow">
        <h1>{t("auto.title")}</h1>
        <p class="sub">{t("auto.intro")}</p>
      </div>
      <a class="btn primary" href="/automations?new" id="btn-new-automation"><Plus size={16} aria-hidden="true" />{t("auto.new")}</a>
    </header>
    {#if loginOff}
      <p class="note" id="auto-login-note">{t("auto.loginOff")} <a href="/settings?s=window">{t("auto.loginOffLink")}</a></p>
    {/if}
    {#if listState === "loading"}
      <p class="hint" role="status">{t("auto.loading")}</p>
    {:else if listState === "error"}
      <div class="state-box" role="alert">
        <h2>{t("auto.loadFailed")}</h2>
        <p>{tb(listError)}</p>
        <button class="btn secondary" type="button" onclick={() => loadList()}>{t("auto.tryAgain")}</button>
      </div>
    {:else if list.length === 0}
      <p class="hint" id="auto-empty">{t("auto.empty")}</p>
    {:else}
      <section class="section" id="automation-list" aria-label={t("auto.title")}>
        {#each list as a (a.id)}
          <AutomationRow {a} busy={running === a.id} onrun={() => runNow(a)} ontoggle={() => toggle(a)} ondelete={() => askDelete(a)} />
        {/each}
      </section>
    {/if}
  {/if}
</main>

<Dialog bind:open={() => deleting !== null, (v) => !v && (deleting = null)} labelledby="auto-del-title">
  {#if deleting}
    <div class="d-body" id="delete-automation-dialog">
      <h2 id="auto-del-title">{t("auto.deleteTitle")}</h2>
      <p class="meta" style="margin:0;font-size:14px">{t("auto.deleteBody", { name: deleting.name })}</p>
      {#if deleteError}<p class="err-text" role="alert" style="margin:0">{deleteError}</p>{/if}
      <div class="d-foot">
        <span class="grow"></span>
        <button class="btn secondary" type="button" onclick={() => (deleting = null)}>{t("auto.keep")}</button>
        <button class="btn danger" type="button" id="btn-confirm-delete-automation" disabled={deleteBusy} onclick={confirmDelete}>
          <Trash size={16} aria-hidden="true" />{deleteBusy ? t("auto.deleting") : t("auto.delete")}
        </button>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
    font-size: 13px;
    font-weight: 700;
    color: var(--ink-2);
  }
  .back:hover {
    color: var(--ink);
  }
  .page-head {
    flex-wrap: wrap;
  }
  /* A link inside a sentence needs more than its color to read as one. */
  .note a {
    font-weight: 700;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .active {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 14px;
    font-weight: 700;
  }
  /* Error red keeps 4.5:1 only with the surface-2 tint behind it (DESIGN.md, contrast table). */
  .danger-text,
  .danger-text:hover {
    color: var(--st-err);
    background: var(--surface-2);
  }
  .auto-wrap {
    max-width: 760px;
  }
  /* The form takes the room, the runs keep the 300 column the session panel has. */
  .auto-detail {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 24px;
    align-items: start;
  }
  .runs {
    gap: 12px;
    padding: 16px;
  }
  .runs h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 800;
  }
  .run-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .run {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }
  .run:first-child {
    padding-top: 0;
    border-top: 0;
  }
  .run-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .run-when {
    font-size: 13px;
    font-weight: 700;
  }
  @media (max-width: 1100px) {
    .auto-detail {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
