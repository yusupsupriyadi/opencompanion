<script lang="ts">
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import ArrowUDownLeft from "phosphor-svelte/lib/ArrowUDownLeft";
  import MagnifyingGlassMinus from "phosphor-svelte/lib/MagnifyingGlassMinus";
  import MagnifyingGlassPlus from "phosphor-svelte/lib/MagnifyingGlassPlus";
  import { onDestroy, tick, untrack } from "svelte";
  import { MEDIA_TOO_LARGE, api, errorText, type FileText, type GitChange, type GitDiff } from "./api";
  import { parseDiff } from "./diff";
  import { LANG_NAME, highlight, langFor, type Lang, type Token } from "./highlight";
  import { plural, t, tb } from "./i18n.svelte";
  import { mediaFor, zoomStep } from "./media";
  import PdfPages from "./PdfPages.svelte";
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

  /** An SVG is a picture and code at once; it opens as the picture, and the code view holds for that file only. */
  let codeFor = $state<string | null>(null);
  const svgCode = $derived(codeFor === target.path);
  const isSvg = $derived(mediaFor(target.path)?.label === "SVG");
  const media = $derived(target.kind === "file" && !(isSvg && svgCode) ? mediaFor(target.path) : null);
  let bytes = $state.raw<ArrayBuffer | null>(null);
  let url = $state("");
  let tooBig = $state(false);
  let shownSig = "";
  /** Null fits the picture or the widest page to the panel; 1 is the file's own size. */
  let zoom = $state<number | null>(null);
  let img: HTMLImageElement | undefined = $state();
  let natural = $state<{ w: number; h: number } | null>(null);
  let pdfFit = $state(1);
  let pdfPages = $state(0);

  const canDiff = $derived(!!change && change.code !== "D" && change.code !== "?");

  function showSvg(code: boolean) {
    codeFor = code ? target.path : null;
    if (target.kind !== "file") onkind("file");
  }

  const viewKey = (kind: "file" | "diff", path: string, asMedia: boolean) => `${kind}:${path}:${asMedia ? "media" : "text"}`;
  const sig = (c: GitChange | undefined) => (c ? `${c.code}:${c.added}:${c.removed}` : "");

  function dropPicture() {
    if (url) URL.revokeObjectURL(url);
    url = "";
    bytes = null;
    natural = null;
    pdfPages = 0;
  }

  async function load(kind: "file" | "diff", path: string, c: GitChange | undefined, asMedia: boolean, force = false) {
    const key = viewKey(kind, path, asMedia);
    // A picture is read again only when git says the file changed or on Refresh: it can be 50 MB.
    if (asMedia && !force && key === shownKey && phase === "ready" && sig(c) === shownSig) return;
    // Reading the same view again keeps it on screen; a different one starts from its loading state.
    if (key !== shownKey) {
      phase = "loading";
      file = null;
      diff = null;
      showAll = false;
      tooBig = false;
      zoom = null;
      dropPicture();
    }
    const stale = () => viewKey(target.kind, target.path, !!media) !== key;
    try {
      if (asMedia) {
        const b = await api.folderReadBytes(id, path);
        if (stale()) return;
        dropPicture();
        tooBig = false;
        bytes = b;
        const m = mediaFor(path);
        if (m?.kind === "image") url = URL.createObjectURL(new Blob([b], { type: m.mime }));
      } else if (kind === "file") {
        const f = await api.folderRead(id, path);
        if (stale()) return;
        file = f;
      } else {
        const d = await api.gitDiff(id, c ?? { path, oldPath: null, code: "M" });
        if (stale()) return;
        diff = d;
      }
      shownKey = key;
      shownSig = sig(c);
      phase = "ready";
    } catch (e) {
      if (stale()) return;
      shownKey = key;
      shownSig = sig(c);
      error = errorText(e);
      // Over the size limit is a state of the file, not a failure to retry.
      tooBig = asMedia && error === MEDIA_TOO_LARGE;
      phase = tooBig ? "ready" : "error";
    }
  }

  const reload = () => load(target.kind, target.path, change, !!media, true);

  $effect(() => {
    // Read again when the view changes and whenever the git status does.
    void version;
    const kind = target.kind;
    const path = target.path;
    const c = change;
    const asMedia = !!media;
    untrack(() => load(kind, path, c, asMedia));
  });
  onDestroy(() => {
    if (url) URL.revokeObjectURL(url);
  });

  function pictureFailed() {
    error = t("workspace.viewer.imageBroken");
    phase = "error";
  }

  function pictureLoaded() {
    if (img) natural = { w: img.naturalWidth || img.clientWidth, h: img.naturalHeight || img.clientHeight };
  }

  function zoomBy(dir: 1 | -1) {
    const fitted = media?.kind === "pdf" ? pdfFit : img && natural?.w ? img.clientWidth / natural.w : 1;
    const next = zoomStep(zoom ?? fitted, dir);
    if (next !== null) zoom = next;
  }

  const zoomLabel = $derived(zoom === null ? t("workspace.viewer.fitLabel") : `${Math.round(zoom * 100)}%`);
  const canZoom = $derived(phase === "ready" && !tooBig && (media?.kind === "pdf" ? pdfPages > 0 : !!url));
  const mediaInfo = $derived.by(() => {
    if (!media || !canZoom) return "";
    if (media.kind === "pdf") return `${media.label} · ${plural(pdfPages, "workspace.viewer.pagesOne", "workspace.viewer.pagesMany")}`;
    return natural ? `${media.label} · ${natural.w} × ${natural.h}` : media.label;
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
  const shownLines = $derived(wantsAll ? lines : lines.slice(0, LINE_LIMIT));
  const shownRows = $derived(wantsAll ? rows : rows.slice(0, LINE_LIMIT));
  const digits = $derived(
    String(target.kind === "file" ? lines.length : rows.reduce((n, r) => ("new" in r ? Math.max(n, r.new ?? 0, r.old ?? 0) : n), 0)).length,
  );
  const hasText = $derived(
    phase === "ready" && (target.kind === "file" ? !!file && !file.binary && !file.tooLarge && lines.length > 0 : rows.length > 0),
  );

  const lang = $derived(langFor(target.path));
  /** Syntax colors per shown row. They arrive after the plain text is on screen; until then a row shows as text. */
  let colors = $state.raw<(Token[] | undefined)[]>([]);
  let painted = "";
  let paintRun = 0;

  type Job = { lines: string[]; rows: number[] };

  function jobs(): Job[] {
    if (target.kind === "file") return [{ lines: shownLines, rows: shownLines.map((_, i) => i) }];
    // Each side of a diff is read as its own run of code, so a removed line colors the way the old file had it.
    const now: Job = { lines: [], rows: [] };
    const before: Job = { lines: [], rows: [] };
    shownRows.forEach((r, i) => {
      if (r.kind === "hunk" || r.kind === "note") return;
      if (r.kind !== "del") {
        now.lines.push(r.text);
        now.rows.push(i);
      }
      if (r.kind !== "add") {
        before.lines.push(r.text);
        before.rows.push(r.kind === "del" ? i : -1);
      }
    });
    return [now, before];
  }

  async function paint(run: number, l: Lang, work: Job[]) {
    const out: (Token[] | undefined)[] = [];
    try {
      for (const job of work) {
        if (job.lines.length === 0) continue;
        await highlight(job.lines, l, (done) => {
          if (run !== paintRun) return false;
          done.forEach((line, k) => {
            if (job.rows[k] >= 0) out[job.rows[k]] = line;
          });
          colors = out.slice();
          return true;
        });
        if (run !== paintRun) return;
      }
    } catch {
      // Without its grammar the file stays plain text, as it shows while the colors load; Refresh tries again.
      if (run === paintRun) painted = "";
    }
  }

  $effect(() => {
    const l = hasText ? lang : null;
    const work = l ? jobs() : [];
    const key = l ? `${target.kind}:${l}\n${work.map((j) => j.lines.join("\n")).join("\n\0\n")}` : "";
    // The same text read again (the git status moved) keeps its colors instead of flashing plain.
    if (key === painted) return;
    painted = key;
    colors = [];
    const run = ++paintRun;
    if (l) paint(run, l, work);
  });
  // A viewer that closes mid-way stops coloring.
  onDestroy(() => paintRun++);

  const WRAP_KEY = "oc-viewer-wrap";
  let wrap = $state(readWrap());

  function readWrap() {
    try {
      return localStorage.getItem(WRAP_KEY) === "1";
    } catch {
      return false;
    }
  }

  function toggleWrap() {
    wrap = !wrap;
    try {
      if (wrap) localStorage.setItem(WRAP_KEY, "1");
      else localStorage.removeItem(WRAP_KEY);
    } catch {
      // Private windows can refuse storage; the choice still holds while this viewer is open.
    }
  }

  // A search hit opens its file at the matching line.
  $effect(() => {
    const line = target.line;
    if (phase !== "ready" || target.kind !== "file" || !line) return;
    tick().then(() => body?.querySelector<HTMLElement>(`[data-line="${line}"]`)?.scrollIntoView?.({ block: "center" }));
  });
</script>

<!-- One line: `.tx` keeps whitespace, so nothing may sit between the tokens. -->
{#snippet code(text: string, tokens: Token[] | undefined)}{#if tokens}{#each tokens as tok, k (k)}<span class:i={tok.italic} class:b={tok.bold} style:color={tok.color || undefined}>{tok.text}</span>{/each}{:else}{text}{/if}{/snippet}

<div class="viewer-bar">
  <span class="path mono" title={target.path}>{target.path}</span>
  {#if hasText}
    <span class="lang">{lang ? LANG_NAME[lang] : t("workspace.viewer.plain")}</span>
  {:else if mediaInfo}
    <span class="lang">{mediaInfo}</span>
  {/if}
  {#if (isSvg && change?.code !== "D") || canDiff}
    <div class="seg dark" role="group" aria-label={t("workspace.viewer.show")}>
      {#if isSvg && change?.code !== "D"}
        <button type="button" aria-pressed={target.kind === "file" && !svgCode} onclick={() => showSvg(false)}>{t("workspace.viewer.preview")}</button>
        <button type="button" aria-pressed={target.kind === "file" && svgCode} onclick={() => showSvg(true)}>{t("workspace.viewer.code")}</button>
      {:else}
        <button type="button" aria-pressed={target.kind === "file"} onclick={() => onkind("file")}>{t("workspace.viewer.showFile")}</button>
      {/if}
      {#if canDiff}
        <button type="button" aria-pressed={target.kind === "diff"} onclick={() => onkind("diff")}>{t("workspace.viewer.showDiff")}</button>
      {/if}
    </div>
  {/if}
  {#if hasText}
    <button class="icon-btn sm" type="button" aria-pressed={wrap} aria-label={t("workspace.viewer.wrap")} title={t("workspace.viewer.wrap")} onclick={toggleWrap}>
      <ArrowUDownLeft size={16} weight={wrap ? "bold" : "regular"} aria-hidden="true" />
    </button>
  {:else if media && canZoom}
    <div class="zoom" role="group" aria-label={t("workspace.viewer.zoom")}>
      <button class="icon-btn sm" type="button" aria-label={t("workspace.viewer.zoomOut")} title={t("workspace.viewer.zoomOut")} disabled={zoom !== null && zoomStep(zoom, -1) === null} onclick={() => zoomBy(-1)}>
        <MagnifyingGlassMinus size={16} aria-hidden="true" />
      </button>
      <button
        class="fit"
        type="button"
        aria-pressed={zoom === null}
        aria-label={zoom === null ? t("workspace.viewer.fit") : t("workspace.viewer.fitNow", { zoom: zoomLabel })}
        title={t("workspace.viewer.fit")}
        onclick={() => (zoom = null)}>{zoomLabel}</button
      >
      <button class="icon-btn sm" type="button" aria-label={t("workspace.viewer.zoomIn")} title={t("workspace.viewer.zoomIn")} disabled={zoom !== null && zoomStep(zoom, 1) === null} onclick={() => zoomBy(1)}>
        <MagnifyingGlassPlus size={16} aria-hidden="true" />
      </button>
    </div>
  {/if}
  <button class="icon-btn sm" type="button" aria-label={t("workspace.refreshNamed", { what: name })} title={t("workspace.refresh")} onclick={reload}>
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
      <button class="btn secondary sm" type="button" onclick={reload}>{t("sessions.tryAgain")}</button>
    </div>
  {:else if media && tooBig}
    <p class="v-note">{t("workspace.viewer.mediaTooLarge", { name })}</p>
  {:else if media?.kind === "image" && url}
    <div class="picture" class:zoomed={zoom !== null && natural} class:sharp={(zoom ?? 0) >= 2}>
      <img
        src={url}
        alt={name}
        bind:this={img}
        onload={pictureLoaded}
        onerror={pictureFailed}
        style:width={zoom !== null && natural ? `${Math.round(natural.w * zoom)}px` : undefined}
      />
    </div>
  {:else if media?.kind === "pdf" && bytes}
    {#if pdfPages === 0}
      <p class="v-note" role="status">{t("workspace.viewer.loading", { name })}</p>
    {/if}
    <PdfPages
      data={bytes}
      {zoom}
      scroller={body}
      bind:fit={pdfFit}
      bind:pages={pdfPages}
      onfail={(message) => {
        error = message;
        phase = "error";
      }}
    />
  {:else if target.kind === "file" && file}
    {#if file.binary}
      <p class="v-note">{t("workspace.viewer.binary", { name })}</p>
    {:else if file.tooLarge}
      <p class="v-note">{t("workspace.viewer.tooLarge", { name })}</p>
    {:else if lines.length === 0}
      <p class="v-note">{t("workspace.viewer.empty", { name })}</p>
    {:else}
      <div class="code" class:wrap style:--digits={digits}>
        {#each shownLines as text, i (i)}
          <div class="cl" class:hit={i + 1 === target.line} data-line={i + 1}>
            <span class="gut" aria-hidden="true"><span class="no">{i + 1}</span></span><span class="tx">{@render code(text, colors[i])}</span>
          </div>
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
      <div class="code diff" class:wrap style:--digits={digits}>
        {#each shownRows as row, i (i)}
          {#if row.kind === "hunk"}
            <div class="cl hunk"><span class="tx">{row.text}</span></div>
          {:else if row.kind === "note"}
            <div class="cl note"><span class="tx">{row.text}</span></div>
          {:else}
            <div class="cl {row.kind}">
              <span class="gut" aria-hidden="true">
                <span class="no">{row.old ?? ""}</span>
                <span class="no">{row.new ?? ""}</span>
                <span class="sign">{row.kind === "add" ? "+" : row.kind === "del" ? "−" : ""}</span>
              </span>
              {#if row.kind !== "ctx"}<span class="sr-only">{row.kind === "add" ? t("workspace.viewer.added") : t("workspace.viewer.removed")}:</span>{/if}
              <span class="tx">{@render code(row.text, colors[i])}</span>
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
  .viewer-bar .icon-btn[aria-pressed="true"] {
    background: #ffffff1f;
    color: var(--term-text);
  }
  .lang {
    flex: none;
    font: 600 12px var(--font-ui);
    color: var(--term-dim);
  }
  .viewer-bar .icon-btn:disabled {
    background: transparent;
    color: var(--term-dim);
    cursor: default;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .fit {
    min-width: 52px;
    height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--r-btn);
    background: transparent;
    color: var(--term-text);
    font: 600 12px var(--font-ui);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }
  .fit:hover {
    background: #ffffff1a;
  }
  .fit[aria-pressed="true"] {
    background: #ffffff1f;
  }
  /* Fitted, a picture never grows past its own size and stays whole in the panel. */
  .picture {
    height: 100%;
    padding: 16px;
    display: grid;
    place-items: center;
  }
  .picture img {
    display: block;
    max-width: 100%;
    max-height: 100%;
    /* Transparent parts show as a checkerboard, so a dark icon with no background is not lost on the dark panel. */
    background: repeating-conic-gradient(#ffffff26 0% 25%, #ffffff0d 0% 50%) 0 0 / 16px 16px;
  }
  .picture.zoomed {
    height: auto;
    min-height: 100%;
    width: max-content;
    min-width: 100%;
  }
  .picture.zoomed img {
    max-width: none;
    max-height: none;
    height: auto;
  }
  /* Zoomed in, pixels stay square, so an icon can be checked pixel by pixel. */
  .picture.sharp img {
    image-rendering: pixelated;
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
    --row: transparent;
    display: flex;
    background: var(--row);
  }
  /* The line numbers stay in view while a long line scrolls sideways, as in an editor. The gutter is opaque, so it
     repeats the row's tint over the panel color. */
  .gut {
    position: sticky;
    left: 0;
    flex: none;
    display: flex;
    background: linear-gradient(var(--row), var(--row)), var(--term-bg);
    box-shadow: inset -1px 0 #ede6c41f;
    user-select: none;
  }
  .no {
    flex: none;
    width: calc(var(--digits) * 1ch + 20px);
    padding-right: 12px;
    text-align: right;
    color: var(--term-dim);
  }
  .sign {
    flex: none;
    width: 2ch;
  }
  .tx {
    padding: 0 20px 0 12px;
    white-space: pre;
  }
  .tx .i {
    font-style: italic;
  }
  .tx .b {
    font-weight: 600;
  }
  .code.wrap {
    width: auto;
  }
  .code.wrap .tx {
    flex: 1;
    min-width: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .cl.add {
    --row: #a6cf6a29;
  }
  .cl.add .sign {
    color: var(--term-green);
  }
  .cl.del {
    --row: #e88b6b29;
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
  /* On the yellow tint the dim color drops under 4.5:1, so the line number and comments take the full text color. */
  .cl.hit {
    --row: #f0c23a2e;
    --syn-comment: var(--term-text);
  }
  .cl.hit .no {
    color: var(--term-text);
  }
</style>
