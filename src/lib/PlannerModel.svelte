<script lang="ts">
  import Brain from "phosphor-svelte/lib/Brain";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import { api, errorText, type ChatModel, type CliInstall, type ModelList, type ModelOption } from "./api";
  import { effortLabel } from "./format";
  import { app, forgetModelLists, plannerModels, showToast } from "./store.svelte";

  /** Model and thinking level for the chat planner, kept per CLI in Settings. */
  let { cli }: { cli: CliInstall } = $props();

  let list = $state<ModelList | null>(null);
  let listState = $state<"loading" | "ready" | "error">("loading");
  let listError = $state("");
  let model = $state("");
  let effort = $state("");
  let saves = 0;

  const saved = $derived<ChatModel>(app.settings?.chatModels?.[cli.kind] ?? { model: "", effort: "" });

  $effect(() => {
    model = saved.model;
    effort = saved.effort;
  });

  async function load(c: CliInstall) {
    listState = "loading";
    try {
      const got = await plannerModels(c);
      if (c.kind !== cli.kind) return;
      list = got;
      listState = "ready";
    } catch (e) {
      if (c.kind !== cli.kind) return;
      list = null;
      listState = "error";
      listError = errorText(e);
    }
  }

  $effect(() => {
    load(cli);
  });

  function retry() {
    forgetModelLists();
    load(cli);
  }

  /** A saved model the list does not have (still loading, or gone after an update) stays pickable. */
  const models = $derived.by<ModelOption[]>(() => {
    const listed = list?.models ?? [];
    if (!saved.model || listed.some((m) => m.id === saved.model)) return listed;
    const label = listState === "loading" ? saved.model : `${saved.model} (saved)`;
    return [{ id: saved.model, label, group: null, efforts: saved.effort ? [saved.effort] : [] }, ...listed];
  });

  const groups = $derived.by(() => {
    const out: { name: string | null; models: ModelOption[] }[] = [];
    for (const m of models) {
      const last = out.at(-1);
      if (last && last.name === m.group) last.models.push(m);
      else out.push({ name: m.group, models: [m] });
    }
    return out;
  });

  function effortsFor(id: string): string[] {
    if (!id) return list?.defaultEfforts ?? (saved.effort ? [saved.effort] : []);
    return models.find((m) => m.id === id)?.efforts ?? [];
  }

  // A saved level the model no longer lists is still what the planner gets, so it stays visible.
  const efforts = $derived.by(() => {
    const listed = effortsFor(model);
    return !effort || listed.includes(effort) ? listed : [...listed, effort];
  });

  // The labels are hidden, so the tooltip names the picker and shows a label the width cut off.
  const modelTitle = $derived(`Model: ${models.find((m) => m.id === model)?.label ?? "CLI default"}`);
  const effortTitle = $derived(
    efforts.length === 0 ? `${cli.label} offers no thinking levels for this model` : `Thinking: ${effort ? effortLabel(effort) : "CLI default"}`,
  );

  async function save() {
    const turn = ++saves;
    try {
      const next = await api.chatSetModel(cli.kind, model, effort);
      if (turn === saves) app.settings = next;
    } catch (e) {
      if (turn !== saves) return;
      model = saved.model;
      effort = saved.effort;
      showToast(`The planner model could not be saved: ${errorText(e)}`);
    }
  }

  function pickModel(e: Event & { currentTarget: HTMLSelectElement }) {
    model = e.currentTarget.value;
    // A level the new model does not take goes back to the CLI default.
    if (!effortsFor(model).includes(effort)) effort = "";
    save();
  }

  function pickEffort(e: Event & { currentTarget: HTMLSelectElement }) {
    effort = e.currentTarget.value;
    save();
  }
</script>

<div class="pickers" id="planner-model">
  <div class="pick">
    <label class="sr-only" for="planner-model-select">Model</label>
    <select
      class="select"
      id="planner-model-select"
      bind:value={model}
      onchange={pickModel}
      aria-busy={listState === "loading"}
      title={modelTitle}
    >
      <option value="">CLI default</option>
      {#each groups as g}
        {#if g.name}
          <optgroup label={g.name}>
            {#each g.models as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
          </optgroup>
        {:else}
          {#each g.models as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
        {/if}
      {/each}
    </select>
    <CaretDown class="caret" size={12} weight="bold" aria-hidden="true" />
  </div>
  <div class="pick lead">
    <Brain class="lead-icon" size={14} aria-hidden="true" />
    <label class="sr-only" for="planner-effort-select">Thinking</label>
    <select
      class="select"
      id="planner-effort-select"
      bind:value={effort}
      onchange={pickEffort}
      disabled={efforts.length === 0}
      title={effortTitle}
    >
      <option value="">CLI default</option>
      {#each efforts as e (e)}<option value={e}>{effortLabel(e)}</option>{/each}
    </select>
    <CaretDown class="caret" size={12} weight="bold" aria-hidden="true" />
  </div>
  {#if listState === "loading"}
    <span class="meta" role="status">Listing {cli.label} models…</span>
  {:else if listState === "error"}
    <span class="err-text" role="alert">{listError}</span>
    <button class="btn ghost sm" type="button" onclick={retry}>List models again</button>
  {/if}
</div>

<style>
  .pickers {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 6px;
    min-width: 0;
  }
  /* Text and a caret, no box: the pickers sit inside the composer and should not compete with Send. */
  .pick {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
    color: var(--ink-2);
  }
  .pick:hover {
    color: var(--ink);
  }
  .pick :global(svg) {
    position: absolute;
    pointer-events: none;
  }
  .pick :global(.caret) {
    right: 8px;
  }
  .pick :global(.lead-icon) {
    left: 8px;
  }
  .pick .select {
    appearance: none;
    field-sizing: content;
    width: auto;
    min-width: 0;
    max-width: 240px;
    min-height: 28px;
    padding: 4px 24px 4px 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: inherit;
    font-size: 13px;
    font-weight: 600;
    text-overflow: ellipsis;
    cursor: pointer;
    transition: background-color 0.12s ease-out;
  }
  .pick.lead .select {
    padding-left: 28px;
  }
  .pick .select:hover:enabled {
    background: var(--surface-2);
  }
  .pick .select option,
  .pick .select optgroup {
    background: var(--surface);
    color: var(--ink);
  }
  .pick .select:disabled {
    cursor: not-allowed;
  }
  .pick:has(.select:disabled) {
    color: var(--ink-2);
  }
  .pick:has(.select:disabled) :global(.caret) {
    display: none;
  }
  .pickers > .meta,
  .pickers > .err-text {
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 720px) {
    .pick .select {
      min-height: 44px;
      max-width: min(240px, 70vw);
    }
    .pickers > .btn.sm {
      min-height: 44px;
    }
  }
</style>
