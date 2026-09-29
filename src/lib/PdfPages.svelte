<script lang="ts">
  import workerSrc from "pdfjs-dist/legacy/build/pdf.worker.min.mjs?url";
  import type { PDFDocumentLoadingTask, PDFDocumentProxy, RenderTask, TextLayer } from "pdfjs-dist/legacy/build/pdf.mjs";
  import { errorText } from "./api";
  import { t } from "./i18n.svelte";

  let {
    data,
    zoom,
    scroller,
    fit = $bindable(1),
    pages = $bindable(0),
    onfail,
  }: {
    data: ArrayBuffer;
    /** Null fits the widest page to the panel; 1 is the printed size. */
    zoom: number | null;
    /** The scrolling box around the pages: pages are drawn only while near its view. */
    scroller: HTMLElement | undefined;
    fit?: number;
    pages?: number;
    onfail: (message: string) => void;
  } = $props();

  /** PDF points are 1/72 inch and CSS pixels 1/96, so 100% shows a page at its printed size. */
  const PT = 96 / 72;
  /** WebKit refuses larger canvases, so a page zoomed past this draws at a lower resolution instead. */
  const MAX_PIXELS = 16_777_216;
  const PAD = 16;

  type Pdfjs = typeof import("pdfjs-dist/legacy/build/pdf.mjs");
  let lib: Pdfjs | undefined;
  let doc = $state.raw<PDFDocumentProxy | null>(null);
  /** Each page's size at scale 1, in points. */
  let sizes = $state.raw<{ w: number; h: number }[]>([]);
  let width = $state(0);
  const scale = $derived((zoom ?? fit) * PT);

  const els = new Map<HTMLElement, number>();
  const near = new Set<HTMLElement>();
  /** The scale each drawn page was drawn at. */
  const drawn = new Map<HTMLElement, number>();
  const work = new Map<HTMLElement, { render?: RenderTask; text?: TextLayer }>();
  let observer: IntersectionObserver | undefined;

  $effect(() => {
    const widest = sizes.reduce((w, s) => Math.max(w, s.w), 0);
    if (widest && width) fit = Math.max(0.1, (width - PAD * 2) / (widest * PT));
  });

  $effect(() => {
    const bytes = data;
    let gone = false;
    let task: PDFDocumentLoadingTask | undefined;
    (async () => {
      try {
        lib ??= await import("pdfjs-dist/legacy/build/pdf.mjs");
        lib.GlobalWorkerOptions.workerSrc = workerSrc;
        // pdf.js hands the bytes to its worker, which empties the buffer: it gets a copy.
        task = lib.getDocument({ data: new Uint8Array(bytes.slice(0)) });
        const d = await task.promise;
        const all = await Promise.all(
          Array.from({ length: d.numPages }, (_, i) =>
            d.getPage(i + 1).then((p) => {
              const v = p.getViewport({ scale: 1 });
              return { w: v.width, h: v.height };
            }),
          ),
        );
        if (gone) return;
        doc = d;
        sizes = all;
        pages = d.numPages;
      } catch (e) {
        if (gone) return;
        onfail(e instanceof Error && e.name === "PasswordException" ? t("workspace.viewer.pdfPassword") : errorText(e));
      }
    })();
    return () => {
      gone = true;
      for (const el of els.keys()) release(el);
      doc = null;
      sizes = [];
      pages = 0;
      void task?.destroy();
    };
  });

  function release(el: HTMLElement) {
    const w = work.get(el);
    w?.render?.cancel();
    w?.text?.cancel();
    work.delete(el);
    drawn.delete(el);
    el.replaceChildren();
  }

  async function draw(el: HTMLElement) {
    const d = doc;
    const i = els.get(el);
    const at = scale;
    if (!d || !lib || i === undefined || drawn.get(el) === at) return;
    work.get(el)?.render?.cancel();
    const page = await d.getPage(i + 1);
    if (d !== doc || !near.has(el) || at !== scale) return;
    const viewport = page.getViewport({ scale: at });
    let ratio = window.devicePixelRatio || 1;
    const area = viewport.width * viewport.height;
    if (area * ratio * ratio > MAX_PIXELS) ratio = Math.sqrt(MAX_PIXELS / area);
    // A fresh canvas, so the old drawing stays (stretched) until the new one is ready.
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(viewport.width * ratio);
    canvas.height = Math.floor(viewport.height * ratio);
    canvas.setAttribute("aria-hidden", "true");
    const render = page.render({ canvas, viewport, transform: ratio !== 1 ? [ratio, 0, 0, ratio, 0, 0] : undefined });
    work.set(el, { render });
    try {
      await render.promise;
    } catch {
      return; // Cancelled: the page scrolled away or the zoom moved on.
    }
    if (d !== doc || !near.has(el) || at !== scale) return;
    // The text sits over the drawing, invisible, so it can be selected and copied.
    const layer = document.createElement("div");
    layer.className = "textLayer";
    el.replaceChildren(canvas, layer);
    drawn.set(el, at);
    const text = new lib.TextLayer({ textContentSource: page.streamTextContent(), container: layer, viewport });
    work.set(el, { text });
    await text.render().catch(() => undefined);
  }

  function seen(entries: IntersectionObserverEntry[]) {
    for (const e of entries) {
      const el = e.target as HTMLElement;
      if (e.isIntersecting) {
        near.add(el);
        void draw(el);
      } else {
        near.delete(el);
        release(el);
      }
    }
  }

  $effect(() => {
    const root = scroller;
    if (!root || typeof IntersectionObserver === "undefined") return;
    // Half a screen above and below, so a page is usually drawn before it scrolls in.
    observer = new IntersectionObserver(seen, { root, rootMargin: "50% 0px" });
    for (const el of els.keys()) observer.observe(el);
    return () => {
      observer?.disconnect();
      observer = undefined;
      near.clear();
    };
  });

  // A new zoom redraws the pages in view and keeps the reading place.
  let place = 0;
  $effect.pre(() => {
    void scale;
    if (scroller) place = scroller.scrollHeight ? scroller.scrollTop / scroller.scrollHeight : 0;
  });
  $effect(() => {
    void scale;
    if (scroller) scroller.scrollTop = place * scroller.scrollHeight;
    for (const el of near) void draw(el);
  });

  function watch(el: HTMLElement, i: number) {
    els.set(el, i);
    observer?.observe(el);
    return {
      destroy() {
        observer?.unobserve(el);
        near.delete(el);
        release(el);
        els.delete(el);
      },
    };
  }
</script>

<div class="pages" bind:clientWidth={width} style:--pad="{PAD}px">
  {#each sizes as s, i (i)}
    <div
      class="page"
      role="group"
      aria-label={t("workspace.viewer.page", { n: i + 1, total: sizes.length })}
      style:width="{Math.floor(s.w * scale)}px"
      style:height="{Math.floor(s.h * scale)}px"
      style:--scale-factor={scale}
      use:watch={i}
    ></div>
  {/each}
</div>

<style>
  .pages {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    width: max-content;
    min-width: 100%;
    padding: var(--pad);
  }
  /* Paper stays white on the dark panel, the color the page is drawn on. */
  .page {
    --total-scale-factor: var(--scale-factor);
    --scale-round-x: 1px;
    --scale-round-y: 1px;
    position: relative;
    flex: none;
    background: #ffffff;
  }
  .page :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  /* pdf.js's text layer, trimmed from its viewer CSS to what a standalone TextLayer needs. */
  .page :global(.textLayer) {
    --min-font-size: 1;
    --text-scale-factor: calc(var(--total-scale-factor) * var(--min-font-size));
    --min-font-size-inv: calc(1 / var(--min-font-size));
    position: absolute;
    inset: 0;
    overflow: clip;
    text-align: initial;
    line-height: 1;
    letter-spacing: normal;
    word-spacing: normal;
    text-size-adjust: none;
    forced-color-adjust: none;
    transform-origin: 0 0;
    z-index: 0;
  }
  .page :global(.textLayer :is(span, br)) {
    color: transparent;
    position: absolute;
    white-space: pre;
    cursor: text;
    transform-origin: 0% 0%;
  }
  .page :global(.textLayer > :not(.markedContent)),
  .page :global(.textLayer .markedContent span:not(.markedContent)) {
    --font-height: 0;
    --scale-x: 1;
    --rotate: 0deg;
    z-index: 1;
    font-size: calc(var(--text-scale-factor) * var(--font-height));
    transform: rotate(var(--rotate)) scaleX(var(--scale-x)) scale(var(--min-font-size-inv));
  }
  .page :global(.textLayer .markedContent) {
    display: contents;
  }
  .page :global(.textLayer ::selection) {
    background: color-mix(in srgb, AccentColor, transparent 50%);
    color: transparent;
  }
  .page :global(.textLayer br::selection) {
    background: transparent;
  }
  .page :global(.textLayer .endOfContent) {
    display: block;
    position: absolute;
    inset: 100% 0 0;
    z-index: 0;
    cursor: default;
    user-select: none;
  }
  .page :global(.textLayer.selecting .endOfContent) {
    top: 0;
  }
  .page :global([data-main-rotation="90"]) {
    transform: rotate(90deg) translateY(-100%);
  }
  .page :global([data-main-rotation="180"]) {
    transform: rotate(180deg) translate(-100%, -100%);
  }
  .page :global([data-main-rotation="270"]) {
    transform: rotate(270deg) translateX(-100%);
  }
</style>
