<script lang="ts">
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import { tick } from "svelte";
  import { api, errorText, type FileText, type GitChange, type GitDiff } from "./api";
  import { parseDiff } from "./diff";
  import { t, tb } from "./i18n.svelte";
  import { splitPath, type ViewTarget } from "./workspace.svelte";

  let {
    id,
    target,
    change,
    version,
    onkind,
  }: {
    id: string;
    target: ViewTarget;
    /** The file's uncommitted change, when it has one: the viewer can then show either. */
    change: GitChange | undefined;
    /** Goes up when the git status changes; what is shown is read again. */
    version: number;
    onkind: (kind: "file" | "diff") => void;
  } = $props();

  /** Lines drawn at first; a longer file shows the rest on request. */
  const LINE_LIMIT = 4000;

  const name = $derived(splitPath(target.path).name);
  let file = $state<FileText | null>(null);
  let diff = $state<GitDiff | null>(null);
  let phase = $state<"loading" | "ready" | "error">("loading");
  let error = $state("");
  let showAll = $state(false);
  let body: HTMLDivElement | undefined = $state();
  let shownKey = "";

  async function load(kind: "file" | "diff", path: string, c: GitChange | undefined) {
    const key = `${kind}:${path}`;
    // Reading the same view again keeps it on screen; a different one starts from its loading state.
    if (key !== shownKey) {
      phase = "loading";
      file = null;
      diff = null;
      showAll = false;
    }
    try {
      if (kind === "file") {
        const f = await api.folderRead(id, path);
        if (`${target.kind}:${target.path}` !== key) return;
        file = f;
      } else {
        const d = await api.gitDiff(id, c ?? { path, oldPath: null, code: "M" });
        if (`${target.kind}:${target.path}` !== key) return;
        diff = d;
      }
      shownKey = key;
      phase = "ready";
    } catch (e) {
      if (`${target.kind}:${target.path}` !== key) return;
      shownKey = key;
      error = errorText(e);
      phase = "error";
    }
  }

  $effect(() => {
    // Read again when the view changes and whenever the git status does.
    void version;
    load(target.kind, target.path, change);
  });

  const lines = $derived.by(() => {
    const text = file?.text ?? "";
    const all = text.split(/\r?\n/);
    if (all.length > 1 && all.at(-1) === "") all.pop();
    return text ? all : [];
  });
  const rows = $derived(diff && !diff.binary && !diff.tooLarge ? parseDiff(diff.patch) : []);
  const total = $derived(target.kind === "file" ? lines.length : rows.length);
  const wantsAll = $derived(showAll || (target.line ?? 0) > LINE_LIMIT);
  const digits = $derived(
    String(target.kind === "file" ? lines.length : rows.reduce((n, r) => ("new" in r ? Math.max(n, r.new ?? 0, r.old ?? 0) : n), 0)).length,
  );

  // A search hit opens its file at the matching line.
  $effect(() => {
    const line = target.line;
    if (phase !== "ready" || target.kind !== "file" || !line) return;
    tick().then(() => body?.querySelector<HTMLElement>(`[data-line="${line}"]`)?.scrollIntoView?.({ block: "center" }));
  });
</script>

<div class="viewer-bar">
  <span class="path mono" title={target.path}>{target.path}</span>
  {#if change && change.code !== "D" && change.code !== "?"}
    <div class="seg dark" role="group" aria-label={t("workspace.viewer.show")}>
      <button type="button" aria-pressed={target.kind === "file"} onclick={() => onkind("file")}>{t("workspace.viewer.showFile")}</button>
      <button type="button" aria-pressed={target.kind === "diff"} onclick={() => onkind("diff")}>{t("workspace.viewer.showDiff")}</button>
    </div>
  {/if}
  <button class="icon-btn sm" type="button" aria-label={t("workspace.refreshNamed", { what: name })} title={t("workspace.refresh")} onclick={() => load(target.kind, target.path, change)}>
    <ArrowClockwise size={16} aria-hidden="true" />
  </button>
</div>

<!-- A scrolling region takes focus so the keyboard can scroll it. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="viewer-body" id="viewer-body" role="region" aria-label={t("workspace.viewer.label", { path: target.path })} tabindex="0" bind:this={body}>
  {#if phase === "loading"}
    <p class="v-note" role="status">{t("workspace.viewer.loading", { name })}</p>
  {:else if phase === "error"}
    <div class="v-note" role="alert">
      <p class="v-err">{t("workspace.viewer.failed", { name })}: {tb(error)}</p>
      <button class="btn secondary sm" type="button" onclick={() => load(target.kind, target.path, change)}>{t("sessions.tryAgain")}</button>
    </div>
  {:else if target.kind === "file" && file}
    {#if file.binary}
      <p class="v-note">{t("workspace.viewer.binary", { name })}</p>
    {:else if file.tooLarge}
      <p class="v-note">{t("workspace.viewer.tooLarge", { name })}</p>
    {:else if lines.length === 0}
      <p class="v-note">{t("workspace.viewer.empty", { name })}</p>
    {:else}
      <div class="code" style:--digits={digits}>
        {#each wantsAll ? lines : lines.slice(0, LINE_LIMIT) as text, i (i)}
          <div class="cl" class:hit={i + 1 === target.line} data-line={i + 1}><span class="no" aria-hidden="true">{i + 1}</span><span class="tx">{text}</span></div>
        {/each}
      </div>
    {/if}
  {:else if target.kind === "diff" && diff}
    {#if diff.tooLarge}
      <p class="v-note">{t("workspace.viewer.diffTooLarge", { name })}</p>
    {:else if diff.binary}
      <p class="v-note">{t("workspace.viewer.binaryDiff", { name })}</p>
    {:else if rows.length === 0}
      <p class="v-note">{t("workspace.viewer.noDiff", { name })}</p>
    {:else}
      <div class="code diff" style:--digits={digits}>
        {#each wantsAll ? rows : rows.slice(0, LINE_LIMIT) as row, i (i)}
          {#if row.kind === "hunk"}
            <div class="cl hunk"><span class="tx">{row.text}</span></div>
          {:else if row.kind === "note"}
            <div class="cl note"><span class="tx">{row.text}</span></div>
          {:else}
            <div class="cl {row.kind}">
              <span class="no" aria-hidden="true">{row.old ?? ""}</span>
              <span class="no" aria-hidden="true">{row.new ?? ""}</span>
              <span class="sign" aria-hidden="true">{row.kind === "add" ? "+" : row.kind === "del" ? "−" : ""}</span>
              {#if row.kind !== "ctx"}<span class="sr-only">{row.kind === "add" ? t("workspace.viewer.added") : t("workspace.viewer.removed")}:</span>{/if}
              <span class="tx">{row.text}</span>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  {/if}
  {#if phase === "ready" && total > LINE_LIMIT && !wantsAll}
    <div class="v-note">
      <button class="btn secondary sm" type="button" onclick={() => (showAll = true)}>{t("workspace.viewer.showAll", { n: total })}</button>
    </div>
  {/if}
</div>

<style>
  .viewer-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 16px;
    border-bottom: 1px solid #ede6c433;
    color: var(--term-text);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    color: var(--term-dim);
  }
  .viewer-bar .icon-btn {
    color: var(--term-dim);
  }
  .viewer-bar .icon-btn:hover {
    background: #ffffff1a;
    color: var(--term-text);
  }
  .viewer-bar :focus-visible,
  .viewer-body:focus-visible,
  .viewer-body :focus-visible {
    outline-color: var(--term-green);
  }
  /* The segmented switch on the dark panel: the terminal's own colors instead of the glass ones. */
  .seg.dark {
    background: #ffffff14;
    box-shadow: none;
  }
  .seg.dark button {
    min-height: 28px;
    padding: 4px 10px;
    color: var(--term-dim);
  }
  .seg.dark button[aria-pressed="true"] {
    background: #ffffff1f;
    color: var(--term-text);
  }
  .viewer-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    font: 13px/1.45 var(--font-mono);
    color: var(--term-text);
  }
  .viewer-body:focus-visible {
    outline-offset: -2px;
  }
  .v-note {
    margin: 0;
    padding: 18px 20px;
    font: 13px/1.45 var(--font-mono);
    color: var(--term-dim);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .v-note p {
    margin: 0;
  }
  .v-err {
    color: var(--term-red);
  }
  /* As wide as the longest line, so the add and remove tints run the full row when scrolled. */
  .code {
    display: flex;
    flex-direction: column;
    width: max-content;
    min-width: 100%;
    padding: 10px 0 18px;
    tab-size: 4;
  }
  /* Whitespace between the spans of a flex row is not drawn, so only the text keeps its spaces. */
  .cl {
    display: flex;
  }
  .no {
    flex: none;
    width: calc(var(--digits) * 1ch + 20px);
    padding-right: 12px;
    text-align: right;
    color: var(--term-dim);
    user-select: none;
  }
  .sign {
    flex: none;
    width: 2ch;
    user-select: none;
  }
  .tx {
    padding-right: 20px;
    white-space: pre;
  }
  .cl.add {
    background: #a6cf6a29;
  }
  .cl.add .sign {
    color: var(--term-green);
  }
  .cl.del {
    background: #e88b6b29;
  }
  .cl.del .sign {
    color: var(--term-red);
  }
  .cl.hunk {
    margin-top: 6px;
    padding: 2px 0 2px calc(var(--digits) * 2ch + 40px);
    background: #ffffff0f;
    color: var(--term-dim);
  }
  .cl.hunk:first-child {
    margin-top: 0;
  }
  .cl.note {
    padding-left: calc(var(--digits) * 2ch + 40px);
    color: var(--term-dim);
    font-style: italic;
  }
  .cl.hit {
    background: #f0c23a2e;
  }
  .cl.hit .no {
    color: var(--term-text);
  }
</style>
