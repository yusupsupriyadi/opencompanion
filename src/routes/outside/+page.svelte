<script lang="ts">
  import { page } from "$app/state";
  import { tick } from "svelte";
  import { api, errorText, type ExternalSession, type Transcript, type TranscriptLine } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import { CLI_LABEL, ago, clock, duration, folderName, memory, shortPath } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { app } from "$lib/store.svelte";

  const ENDED = "This process has ended.";
  const pid = $derived(Number(page.url.searchParams.get("pid")));

  let found = $state<ExternalSession | null>(null);
  let transcript = $state<Transcript | null>(null);
  let loadState = $state<"loading" | "ready" | "ended" | "error">("loading");
  let loadError = $state("");
  let attempt = $state(0);
  let sideOpen = $state(false);
  let box: HTMLPreElement | undefined = $state();

  // The periodic scan also measures CPU; the detail call only confirms the process and reads memory.
  const scanned = $derived(found ? app.outside.find((o) => o.pid === found?.pid && o.startedAt === found.startedAt) : undefined);
  const x = $derived(scanned ?? found);
  const title = $derived(x ? (x.cwd ? t("sessions.outside.title", { cli: CLI_LABEL[x.kind], folder: folderName(x.cwd) }) : CLI_LABEL[x.kind]) : "");
  const last = $derived(transcript?.lines.findLast((l) => l.at !== null)?.at ?? null);

  const PREFIX: Record<TranscriptLine["speaker"], string> = { you: "› ", cli: "", tool: "• " };
  const CLS: Record<TranscriptLine["speaker"], string> = { you: "t-g", cli: "", tool: "t-d" };

  async function show(next: Transcript) {
    // Follow new lines only while the reader is at the end, so scrolling back to read is not interrupted.
    const atEnd = !box || box.scrollHeight - box.scrollTop - box.clientHeight < 24;
    transcript = next;
    await tick();
    if (box && atEnd) box.scrollTop = box.scrollHeight;
  }

  // The CLI keeps writing its history while it runs, so the transcript is read again every few seconds.
  $effect(() => {
    const target = pid;
    void attempt;
    found = null;
    transcript = null;
    loadState = "loading";
    if (!Number.isInteger(target) || target <= 0) {
      loadState = "ended";
      return;
    }
    let gone = false;
    let timer: ReturnType<typeof setInterval> | undefined;
    let startedAt: number | null = null;
    const read = async () => {
      try {
        const d = await api.outsideDetail(target);
        if (gone) return;
        // The same process id on another start time is a different process.
        if (startedAt !== null && d.session.startedAt !== startedAt) throw ENDED;
        startedAt = d.session.startedAt;
        found = d.session;
        loadState = "ready";
        await show(d.transcript);
      } catch (e) {
        if (gone) return;
        const msg = errorText(e);
        if (msg === ENDED) {
          loadState = "ended";
          clearInterval(timer);
        } else if (loadState !== "ready") {
          // A failed refresh keeps the transcript already shown.
          loadState = "error";
          loadError = msg;
        }
      }
    };
    read();
    timer = setInterval(read, 5000);
    return () => {
      gone = true;
      clearInterval(timer);
    };
  });
</script>

<svelte:head><title>{title || t("sessions.outside.pageTitle")} · OpenCompanion</title></svelte:head>

<main class="main detail" id="outside-main">
  {#if loadState === "loading"}
    <p class="hint" role="status">{t("sessions.outside.loading")}</p>
  {:else if loadState === "error"}
    <div class="state-box" role="alert">
      <h2>{t("sessions.outside.readFailed")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={() => attempt++}>{t("sessions.tryAgain")}</button>
    </div>
  {:else if loadState === "ended" || !x}
    <div class="not-found" id="outside-ended">
      <p class="crumb"><a href="/">{t("sessions.overview.title")}</a></p>
      <h1>{t("sessions.outside.endedTitle")}</h1>
      <p class="sub">{t("sessions.outside.endedBody")}</p>
      <a class="btn secondary" href="/">{t("sessions.backToOverview")}</a>
    </div>
  {:else}
    <header class="page-head">
      <div class="grow head-stack">
        <nav class="crumb" aria-label={t("sessions.breadcrumb")}><a href="/">{t("sessions.overview.title")}</a> / {x.cwd ? folderName(x.cwd) : t("sessions.folderUnknown")}</nav>
        <div class="row title-row">
          <CliMark kind={x.kind} />
          <h1>{title}</h1>
          <span class="chip ro">{t("sessions.readOnly")}</span>
        </div>
        <div class="meta">
          {t("sessions.openedOutside")} · {t(`sessions.mode.${x.mode}`)} · <span class="mono" title={x.cwd ?? ""}>{x.cwd ? shortPath(x.cwd) : t("sessions.folderUnknown")}</span> · {t("sessions.startedAt", { time: clock(x.startedAt * 1000) })}
        </div>
      </div>
      <button class="btn secondary sm side-toggle" type="button" aria-expanded={sideOpen} aria-controls="outside-side" onclick={() => (sideOpen = !sideOpen)}>
        {sideOpen ? t("sessions.hideDetails") : t("sessions.details")}
      </button>
    </header>

    <div class="detail-body">
      <section class="term" aria-label={t("sessions.outside.transcriptOf", { title })}>
        <pre class="term-out" bind:this={box}>{#if !transcript || transcript.lines.length === 0}<span class="t-d">{transcript?.note ? tb(transcript.note) : t("sessions.outside.readingTranscript")}</span>{:else}{#each transcript.lines as l, i (i)}{#if i > 0}{"\n"}{/if}<span class={CLS[l.speaker]}>{PREFIX[l.speaker]}{l.text}</span>{/each}{/if}</pre>
        <div class="term-note">{t("sessions.outside.transcriptNote")}</div>
      </section>

      <aside class="detail-side" id="outside-side" class:open={sideOpen}>
        <div class="side-group">
          <h3>{t("sessions.process")}</h3>
          <div class="kv"><span>{t("sessions.pid")}</span><b class="mono">{x.pid}</b></div>
          <div class="kv"><span>{t("sessions.runningFor")}</span><b>{duration(app.now - x.startedAt * 1000)}</b></div>
          <div class="kv"><span>{t("sessions.memory")}</span><b class="mono">{memory(x.memoryBytes)}</b></div>
          {#if scanned}<div class="kv"><span>{t("sessions.cpu")}</span><b class="mono">{scanned.cpuPercent}%</b></div>{/if}
        </div>
        <div class="side-group">
          <h3>{t("sessions.outside.transcript")}</h3>
          {#if transcript?.source}
            <p class="mono source" title={transcript.source}>{shortPath(transcript.source)}</p>
            <p>
              {last ? t("sessions.outside.lastEntry", { ago: ago(last, app.now) }) : ""} {t("sessions.outside.readEvery", { cli: CLI_LABEL[x.kind] })}
            </p>
          {:else}
            <p>{transcript?.note ? tb(transcript.note) : t("sessions.outside.lookingForHistory")}</p>
          {/if}
        </div>
        <div class="side-group">
          <h3>{t("sessions.outside.input")}</h3>
          <p>{t("sessions.outside.inputBody")}</p>
        </div>
      </aside>
    </div>
  {/if}
</main>

<style>
  .source {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
</style>
