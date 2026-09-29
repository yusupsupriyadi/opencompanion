<script lang="ts">
  import { untrack } from "svelte";
  import { errorText } from "./api";
  import { t, tb } from "./i18n.svelte";
  import { HOUR_STEPS, MINUTE_STEPS, WEEK, dayLong, dayShort, fromCron, toCron, whenText, type ScheduleKind, type Simple, type Unit } from "./schedule";

  // The schedule part of the automation forms, on the desktop and the phone. `value` is the cron
  // expression; Some days with no day picked leaves it empty. `preview` asks the desktop for the
  // next times, so the owner sees what a schedule means before saving it.
  let {
    value = $bindable(""),
    idPrefix,
    preview,
  }: {
    value?: string;
    idPrefix: string;
    preview: (schedule: string) => Promise<number[]>;
  } = $props();

  const start = untrack(() => value);
  const simple = fromCron(start);
  let kind = $state<ScheduleKind>(simple?.kind ?? (start.trim() ? "cron" : "daily"));
  let time = $state(simple && simple.kind !== "every" ? simple.time : "09:00");
  let days = $state<number[]>(simple?.kind === "days" ? simple.days : [1, 2, 3, 4, 5]);
  let unit = $state<Unit>(simple?.kind === "every" ? simple.unit : "hours");
  let n = $state(simple?.kind === "every" ? simple.n : 1);
  let cron = $state(simple ? "" : start.trim() || "0 9 * * *");

  const KINDS: { id: ScheduleKind; key: "auto.sched.daily" | "auto.sched.days" | "auto.sched.every" | "auto.sched.cron" }[] = [
    { id: "daily", key: "auto.sched.daily" },
    { id: "days", key: "auto.sched.days" },
    { id: "every", key: "auto.sched.every" },
    { id: "cron", key: "auto.sched.cron" },
  ];

  function current(): Simple | null {
    if (kind === "daily") return { kind, time };
    if (kind === "days") return { kind, days, time };
    if (kind === "every") return { kind, n, unit };
    return null;
  }

  const expr = $derived(kind === "cron" ? cron.trim().split(/\s+/).join(" ") : toCron(current()!));
  $effect(() => {
    value = expr;
  });

  /** Cron starts from what the simple mode said, so switching to it never loses the time. */
  function pick(next: ScheduleKind) {
    if (next === "cron" && kind !== "cron") cron = expr || "0 9 * * *";
    if (next !== "cron" && kind === "cron") {
      const s = fromCron(cron);
      if (s && s.kind === next) {
        if (s.kind === "every") [n, unit] = [s.n, s.unit];
        else time = s.time;
        if (s.kind === "days") days = s.days;
      }
    }
    kind = next;
  }

  function toggleDay(d: number) {
    days = days.includes(d) ? days.filter((x) => x !== d) : [...days, d];
  }

  function setUnit(next: Unit) {
    unit = next;
    const steps = next === "minutes" ? MINUTE_STEPS : HOUR_STEPS;
    if (!steps.includes(n)) n = steps[0];
  }

  let times = $state<number[]>([]);
  let previewError = $state("");
  let checking = $state(false);

  // Waits a moment after the last change, so typing a cron expression is one question, not one per key.
  $effect(() => {
    const schedule = expr;
    times = [];
    previewError = "";
    if (!schedule) return;
    checking = true;
    let stale = false;
    const timer = setTimeout(async () => {
      try {
        const got = await preview(schedule);
        if (!stale) times = got;
      } catch (e) {
        if (!stale) previewError = errorText(e);
      } finally {
        if (!stale) checking = false;
      }
    }, 250);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  const steps = $derived(unit === "minutes" ? MINUTE_STEPS : HOUR_STEPS);
  const noDay = $derived(kind === "days" && days.length === 0);
</script>

<fieldset class="field bare sched" id="{idPrefix}-schedule" aria-describedby="{idPrefix}-sched-next">
  <legend class="label">{t("auto.sched.label")}</legend>

  <div class="kinds">
    {#each KINDS as k (k.id)}
      <label>
        <input class="sr-only" type="radio" name="{idPrefix}-sched-kind" value={k.id} checked={kind === k.id} onchange={() => pick(k.id)} />
        {t(k.key)}
      </label>
    {/each}
  </div>

  {#if kind === "days"}
    <div class="sub" role="group" aria-labelledby="{idPrefix}-days-label" aria-describedby={noDay ? `${idPrefix}-days-error` : undefined}>
      <span class="sub-label" id="{idPrefix}-days-label">{t("auto.sched.dayList")}</span>
      <div class="days">
        {#each WEEK as d (d)}
          <button
            type="button"
            class="day"
            id="{idPrefix}-day-{d}"
            aria-pressed={days.includes(d)}
            aria-label={dayLong(d)}
            title={dayLong(d)}
            onclick={() => toggleDay(d)}>{dayShort(d)}</button
          >
        {/each}
      </div>
      {#if noDay}<p class="error" id="{idPrefix}-days-error">{t("auto.sched.pickDay")}</p>{/if}
    </div>
  {/if}

  {#if kind === "daily" || kind === "days"}
    <div class="sub">
      <label class="sub-label" for="{idPrefix}-time">{t("auto.sched.time")}</label>
      <input class="input time" type="time" id="{idPrefix}-time" bind:value={time} required />
    </div>
  {:else if kind === "every"}
    <div class="sub">
      <span class="sub-label" id="{idPrefix}-every-label">{t("auto.sched.interval")}</span>
      <div class="every" role="group" aria-labelledby="{idPrefix}-every-label">
        <select class="select" id="{idPrefix}-every-n" aria-label={t("auto.sched.interval")} bind:value={n}>
          {#each steps as s (s)}<option value={s}>{s}</option>{/each}
        </select>
        <select
          class="select"
          id="{idPrefix}-every-unit"
          aria-label={t("auto.sched.interval")}
          value={unit}
          onchange={(e) => setUnit(e.currentTarget.value as Unit)}
        >
          <option value="minutes">{t("auto.sched.minutes")}</option>
          <option value="hours">{t("auto.sched.hours")}</option>
        </select>
      </div>
    </div>
  {:else}
    <div class="sub">
      <label class="sub-label" for="{idPrefix}-cron">{t("auto.sched.cronLabel")}</label>
      <input
        class="input mono"
        id="{idPrefix}-cron"
        bind:value={cron}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        aria-invalid={previewError ? "true" : undefined}
        aria-describedby="{idPrefix}-cron-help {idPrefix}-sched-next"
      />
      <p class="help" id="{idPrefix}-cron-help">{t("auto.sched.cronHelp")}</p>
    </div>
  {/if}

  <div id="{idPrefix}-sched-next" aria-live="polite">
    {#if previewError}
      <p class="error">{tb(previewError)}</p>
    {:else if times.length}
      <p class="help">{t("auto.sched.next", { list: times.map((at) => whenText(at)).join(", ") })}</p>
    {:else if checking}
      <p class="help">{t("auto.sched.checking")}</p>
    {/if}
  </div>
  <p class="help">{t("auto.sched.local")}</p>
</fieldset>

<style>
  .bare {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .bare legend {
    padding: 0;
    margin-bottom: 8px;
    font-weight: 800;
  }
  /* Four radios drawn as one segmented control, like the tabs of the session panel. */
  .kinds {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-btn);
    background: var(--surface-2);
  }
  .kinds label {
    display: grid;
    place-items: center;
    min-height: 36px;
    padding: 4px 6px;
    border-radius: var(--r-sm);
    color: var(--ink-2);
    font-size: 13px;
    font-weight: 700;
    text-align: center;
    cursor: pointer;
  }
  .kinds label:hover {
    background: var(--surface);
  }
  .kinds label:has(input:checked) {
    background: var(--surface);
    color: var(--ink);
    box-shadow: var(--glass-rim);
  }
  .kinds label:has(input:focus-visible) {
    outline: 2px solid var(--forest);
    outline-offset: 2px;
  }
  .sub {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
  }
  .sub-label {
    font-size: 13px;
    font-weight: 700;
  }
  .days {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
  }
  .day {
    min-height: 36px;
    padding: 0 4px;
    border-radius: var(--r-sm);
    border: 1px solid var(--line-strong);
    background: var(--surface);
    color: var(--ink);
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
  }
  .day:hover {
    background: var(--surface-2);
  }
  .day[aria-pressed="true"] {
    background: var(--forest);
    border-color: var(--forest);
    color: var(--on-forest);
  }
  .time {
    width: min(160px, 100%);
  }
  .every {
    display: flex;
    gap: 8px;
  }
  .every .select {
    width: auto;
    min-width: 96px;
  }
  .sched .help,
  .sched .error {
    margin: 8px 0 0;
  }
  @media (max-width: 720px) {
    .kinds label,
    .day {
      min-height: 44px;
    }
  }
</style>
