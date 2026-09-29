<script lang="ts">
  import Plus from "phosphor-svelte/lib/Plus";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import { onMount } from "svelte";
  import type { Automation, AutomationView } from "$lib/api";
  import AppLogo from "$lib/AppLogo.svelte";
  import CliMark from "$lib/CliMark.svelte";
  import { folderName } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import OutcomeChip from "$lib/OutcomeChip.svelte";
  import { call, notify, onMessage } from "$lib/phone.svelte";
  import { describe, whenText } from "$lib/schedule";

  let list = $state<AutomationView[]>([]);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let now = $state(Date.now());

  async function load(quiet = false) {
    if (!quiet) loadState = "loading";
    try {
      list = (await call<{ automations: AutomationView[] }>("/api/automations")).automations;
      loadState = "ready";
    } catch (e) {
      if (quiet) return;
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggle(a: AutomationView) {
    try {
      const r = await call<{ automation: Automation }>(`/api/automations/${a.id}/enabled`, {
        method: "POST",
        body: JSON.stringify({ enabled: !a.enabled }),
      });
      const saved = r.automation;
      notify(
        saved.enabled && saved.nextRunAt
          ? t("auto.toast.resumed", { name: saved.name, when: whenText(saved.nextRunAt) })
          : t("auto.toast.paused", { name: saved.name }),
      );
    } catch (e) {
      notify(tb(e instanceof Error ? e.message : String(e)));
    }
    load(true);
  }

  onMount(() => {
    load();
    // Changed on the desktop, here, or by a run.
    const off = onMessage((m) => {
      if (m.type === "automations" || m.type === "resync") load(true);
    });
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => {
      off();
      clearInterval(timer);
    };
  });
</script>

<svelte:head><title>{t("auto.title")} · OpenCompanion</title></svelte:head>

<header class="bar"><span class="brand grow"><AppLogo />OpenCompanion</span></header>
<main class="content" id="phone-automations">
  <div class="head-row">
    <h1 class="m-h1">{t("auto.title")}</h1>
    <a class="m-icon-btn lg" href="/m/automations/edit?new" id="btn-new-automation" aria-label={t("auto.new")} title={t("auto.new")}>
      <Plus size={20} aria-hidden="true" />
    </a>
  </div>

  {#if loadState === "loading"}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("auto.m.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{tb(loadError)}</p>
    <button class="btn secondary block" type="button" onclick={() => load()}>{t("phone.tryAgain")}</button>
  {:else if list.length === 0}
    <p class="m-p">{t("auto.empty")}</p>
    <a class="btn primary block" href="/m/automations/edit?new">
      <Plus size={16} aria-hidden="true" />{t("auto.new")}
    </a>
  {:else}
    <section class="m-auto-list" aria-label={t("auto.title")}>
      {#each list as a (a.id)}
        <div class="m-auto">
          <a class="m-card m-press" href="/m/automations/edit?id={a.id}">
            <span class="row"><CliMark kind={a.cli} small /><span class="task grow">{a.name}</span></span>
            <span class="meta">{describe(a.schedule)} · {folderName(a.cwd)}</span>
            <span class="m-auto-foot">
              <span class="m-auto-next">{a.enabled && a.nextRunAt ? t("auto.nextRun", { when: whenText(a.nextRunAt, now) }) : t("auto.paused")}</span>
              {#if a.lastRun}<OutcomeChip outcome={a.lastRun.outcome} />{/if}
            </span>
          </a>
          <!-- Outside the card link, which would open the automation too. -->
          <span class="m-auto-switch">
            <button class="switch" type="button" role="switch" aria-checked={a.enabled} aria-label={t("auto.activeNamed", { name: a.name })} onclick={() => toggle(a)}></button>
          </span>
        </div>
      {/each}
    </section>
  {/if}
</main>

<style>
  .m-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-auto-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .m-auto {
    position: relative;
  }
  /* Room on the right for the switch, so the name never runs under it. */
  .m-auto .m-card {
    padding-right: 70px;
  }
  .m-auto-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 34px;
    font-size: 12px;
    font-weight: 700;
  }
  .m-auto-next {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-auto-switch {
    position: absolute;
    top: 50%;
    right: 12px;
    translate: 0 -50%;
    display: flex;
  }
  /* The switch is 26 tall; the invisible edge around it makes the 44 px touch target. */
  .m-auto-switch .switch {
    position: relative;
  }
  .m-auto-switch .switch::before {
    content: "";
    position: absolute;
    inset: -10px -8px;
  }
</style>
