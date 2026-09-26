<script lang="ts">
  import Check from "phosphor-svelte/lib/Check";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import Plus from "phosphor-svelte/lib/Plus";
  import X from "phosphor-svelte/lib/X";
  import CliMark from "$lib/CliMark.svelte";
  import StatusChip from "$lib/StatusChip.svelte";
  import { CLI_LABEL, ago, folderName, isLive, isToday, trackLine, waitingTitle } from "$lib/format";
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
  const running = $derived(phone.sessions.filter((s) => isLive(s) && s.status !== "waiting"));
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

<header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-sessions">
  <div class="head-row">
    <h1 class="m-h1">{t("phone.nav.sessions")}</h1>
    <a class="btn secondary" href="/m/new" id="btn-new-session"><Plus size={16} aria-hidden="true" />{t("phone.newSession")}</a>
  </div>
  <p class="conn">{phone.connection === "online" ? t("phone.sessions.online") : t("phone.sessions.connecting")}</p>

  {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

  {#if !phone.loaded}
    <p class="m-p" role="status">{t("phone.sessions.loading")}</p>
  {:else if phone.sessions.length === 0}
    <p class="m-p">{t("phone.sessions.empty")}</p>
  {/if}

  {#each waiting as s (s.id)}
    <section class="m-needs" aria-labelledby="mn-{s.id}">
      <div class="row">
        <CliMark kind={s.cli} />
        <div class="grow">
          <h2 id="mn-{s.id}">{waitingTitle(s)}</h2>
          <div class="meta">{folderName(s.cwd)}{s.waiting ? ` · ${t("phone.sessions.asked", { ago: ago(s.waiting.since, now) })}` : ""}</div>
        </div>
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
      <a class="btn secondary" href="/m/session?id={s.id}">{t("phone.openSession")}</a>
    </section>
  {/each}

  {#if running.length}
    <section class="m-sec" aria-labelledby="m-running">
      <h2 id="m-running">{t("phone.sessions.running")}</h2>
      {#each running as s (s.id)}
        <a class="m-card" href="/m/session?id={s.id}">
          <div class="row"><CliMark kind={s.cli} /><span class="task grow">{s.title}</span><StatusChip status={s.status} /></div>
          <span class="meta">{CLI_LABEL[s.cli]} · {folderName(s.cwd)} · {trackLine(s, now)}</span>
        </a>
      {/each}
    </section>
  {/if}

  {#if today.length}
    <section class="m-sec" aria-labelledby="m-today">
      <h2 id="m-today">{t("phone.sessions.today")}</h2>
      {#each today as s (s.id)}
        <a class="m-card" href="/m/session?id={s.id}">
          <div class="row"><CliMark kind={s.cli} /><span class="task grow">{s.title}</span><StatusChip status={s.status} /></div>
          <span class="meta">{CLI_LABEL[s.cli]} · {folderName(s.cwd)} · {trackLine(s, now)}</span>
        </a>
      {/each}
    </section>
  {/if}
</main>
