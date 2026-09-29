<script lang="ts">
  import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
  import Check from "phosphor-svelte/lib/Check";
  import Plus from "phosphor-svelte/lib/Plus";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import X from "phosphor-svelte/lib/X";
  import CliMark from "$lib/CliMark.svelte";
  import PhoneStatus from "$lib/PhoneStatus.svelte";
  import PhoneTopBar from "$lib/PhoneTopBar.svelte";
  import { CLI_LABEL, ago, folderName, isLive, isToday, runsCli, trackLine, waitingTitle } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { answer as sendAnswer, phone } from "$lib/phone.svelte";

  let busy = $state<string | null>(null);
  let failure = $state("");
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(timer);
  });

  const waiting = $derived(phone.sessions.filter((s) => s.status === "waiting"));
  const running = $derived(phone.sessions.filter((s) => runsCli(s) && s.status !== "waiting"));
  // Terminals left at their shell prompt: open on the computer, but nothing runs in them.
  const atPrompt = $derived(phone.sessions.filter((s) => s.status === "shell"));
  const today = $derived(phone.sessions.filter((s) => !isLive(s) && isToday(s.endedAt ?? s.startedAt)));

  async function answer(id: string, allow: boolean) {
    busy = id;
    failure = "";
    try {
      await sendAnswer(id, allow);
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    } finally {
      busy = null;
    }
  }
</script>

<svelte:head><title>{t("phone.nav.sessions")} · OpenCompanion</title></svelte:head>

<PhoneTopBar />
<main class="content dense" id="phone-sessions">
  <div class="head-row">
    <h1 class="m-h1">{t("phone.nav.sessions")}</h1>
    <a class="m-icon-btn secondary" href="/m/new" id="btn-new-session" aria-label={t("phone.newSession")} title={t("phone.newSession")}><Plus size={20} aria-hidden="true" /></a>
  </div>

  {#if failure}<p class="err-text m-appear" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  {#if !phone.loaded}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("phone.sessions.loading")}</p>
  {:else if phone.sessions.length === 0}
    <p class="m-p">{t("phone.sessions.empty")}</p>
    <a class="btn primary block" href="/m/new"><Plus size={16} aria-hidden="true" />{t("phone.newSession")}</a>
  {/if}

  {#each waiting as s (s.id)}
    <section class="m-needs m-appear" aria-labelledby="mn-{s.id}">
      <div class="row m-needs-head">
        <CliMark kind={s.cli} small />
        <div class="grow">
          <h2 id="mn-{s.id}">{waitingTitle(s)}</h2>
          <div class="meta">{folderName(s.cwd)}{s.waiting ? ` · ${t("phone.sessions.asked", { ago: ago(s.waiting.since, now) })}` : ""}</div>
        </div>
        <a class="m-icon-btn" href="/m/session?id={s.id}" aria-label={t("phone.openSession")} title={t("phone.openSession")}><ArrowSquareOut size={18} aria-hidden="true" /></a>
      </div>
      {#if s.waiting?.detail}<div class="cmd"><small>{s.waiting.tool ?? t("phone.request")}</small><code>{s.waiting.detail}</code></div>{/if}
      {#if s.waiting?.canAnswer}
        <div class="pair-btns">
          <button class="btn accent" type="button" disabled={busy === s.id} onclick={() => answer(s.id, true)}><Check size={16} aria-hidden="true" />{t("phone.approve")}</button>
          <button class="btn secondary" type="button" disabled={busy === s.id} onclick={() => answer(s.id, false)}><X size={16} aria-hidden="true" />{t("phone.deny")}</button>
        </div>
      {:else}
        <p class="small" style="margin:0">{t("phone.answerOnComputer")}</p>
      {/if}
    </section>
  {/each}

  {#if running.length}
    <section class="m-sec" aria-labelledby="m-running">
      <h2 id="m-running">{t("phone.sessions.running")} · {running.length}</h2>
      {#each running as s (s.id)}
        <a class="m-card m-press" href="/m/session?id={s.id}">
          <span class="row"><CliMark kind={s.cli} small /><span class="task grow">{s.title}</span><PhoneStatus status={s.status} /></span>
          <span class="meta">{CLI_LABEL[s.cli]} · {folderName(s.cwd)} · {trackLine(s, now)}</span>
        </a>
      {/each}
    </section>
  {/if}

  {#if atPrompt.length}
    <section class="m-sec" aria-labelledby="m-at-prompt">
      <h2 id="m-at-prompt">{t("shell.status.shell")} · {atPrompt.length}</h2>
      {#each atPrompt as s (s.id)}
        <a class="m-card m-press" href="/m/session?id={s.id}">
          <span class="row"><CliMark kind={s.cli} small /><span class="task grow">{s.title}</span><PhoneStatus status={s.status} /></span>
          <span class="meta">{CLI_LABEL[s.cli]} · {folderName(s.cwd)} · {trackLine(s, now)}</span>
        </a>
      {/each}
    </section>
  {/if}

  {#if today.length}
    <section class="m-sec" aria-labelledby="m-today">
      <h2 id="m-today">{t("phone.sessions.today")} · {today.length}</h2>
      {#each today as s (s.id)}
        <a class="m-card m-press" href="/m/session?id={s.id}">
          <span class="row"><CliMark kind={s.cli} small /><span class="task grow">{s.title}</span><PhoneStatus status={s.status} /></span>
          <span class="meta">{CLI_LABEL[s.cli]} · {folderName(s.cwd)} · {trackLine(s, now)}</span>
        </a>
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
  .m-needs-head {
    gap: 10px;
    align-items: flex-start;
  }
  .m-needs-head .meta {
    font-size: 12px;
  }
  .m-needs-head .m-icon-btn {
    margin: -6px -6px 0 0;
  }
</style>
