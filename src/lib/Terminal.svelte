<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount } from "svelte";
  import { api } from "./api";
  import { t } from "./i18n.svelte";

  let { id, live, label }: { id: string; live: boolean; label: string } = $props();

  let host: HTMLDivElement | undefined = $state();
  // Read inside the xterm callbacks, which outlive the first render.
  let acceptInput = false;
  $effect(() => {
    acceptInput = live;
  });

  // Rust answers ConPTY's cursor query itself (docs/spike/M0-results.md), so xterm's own
  // answer is dropped instead of being typed into the CLI.
  const CURSOR_REPORT = /\x1b\[\d+;\d+R/g;

  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  /** Moves focus to the control after (or before) the terminal, the way Tab would without it. */
  function leave(from: HTMLElement, back: boolean) {
    const all = [...document.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => !from.contains(el) && el.getClientRects().length > 0);
    const after = (el: HTMLElement) => Boolean(from.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING);
    const target = back ? all.filter((el) => !after(el)).at(-1) : all.find(after);
    target?.focus();
  }

  onMount(() => {
    acceptInput = live;
    if (!host) return;
    const box = host;
    // xterm keeps its screen reader strings in one global; the input label is applied when the terminal opens.
    Terminal.strings.promptLabel = t("sessions.terminal.input");
    Terminal.strings.tooMuchOutput = t("sessions.terminal.tooMuchOutput");
    const term = new Terminal({
      fontFamily: '"IBM Plex Mono", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      scrollback: 8000,
      allowTransparency: true,
      // DESIGN.md section 12: output reaches screen readers too.
      screenReaderMode: true,
      theme: {
        background: "#00000000",
        foreground: "#EDE6C4",
        cursor: "#A6CF6A",
        cursorAccent: "#1E2B1F",
        selectionBackground: "#A6CF6A55",
        // Same slider as the page scrollbars inside .term: term-dim at 60%, full on hover, term-text while dragged.
        scrollbarSliderBackground: "#A9B38F99",
        scrollbarSliderHoverBackground: "#A9B38F",
        scrollbarSliderActiveBackground: "#EDE6C4",
        black: "#1E2B1F",
        red: "#E88B6B",
        green: "#A6CF6A",
        yellow: "#F0C23A",
        blue: "#7FAEDD",
        magenta: "#D8A7C8",
        cyan: "#8FCFC0",
        white: "#EDE6C4",
        brightBlack: "#A9B38F",
        brightRed: "#F2A58A",
        brightGreen: "#BEE08A",
        brightYellow: "#F7D56E",
        brightBlue: "#A2C6EA",
        brightMagenta: "#E6C0DA",
        brightCyan: "#AEDFD3",
        brightWhite: "#FFF8E1",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(box);
    try {
      fit.fit();
    } catch {
      // A hidden host has no size yet; the observer fits it once it is shown.
    }

    // Tab belongs to the CLI while it runs, so Ctrl+Tab and Ctrl+Shift+Tab leave the terminal
    // instead. A closed terminal takes no keys, and Tab moves on as everywhere else.
    term.attachCustomKeyEventHandler((e) => {
      if (e.key !== "Tab") return true;
      if (!acceptInput) return false;
      if (!e.ctrlKey) return true;
      if (e.type === "keydown") {
        e.preventDefault();
        leave(box, e.shiftKey);
      }
      return false;
    });

    let chain: Promise<unknown> = Promise.resolve();
    const send = (data: string) => {
      chain = chain.then(() => api.sendInput(id, data)).catch(() => undefined);
    };
    term.onData((d) => {
      if (!acceptInput) return;
      const clean = d.replace(CURSOR_REPORT, "");
      if (clean) send(clean);
    });
    term.onResize(({ cols, rows }) => {
      if (acceptInput) api.resizeSession(id, cols, rows).catch(() => undefined);
    });
    if (acceptInput) api.resizeSession(id, term.cols, term.rows).catch(() => undefined);

    // Listening starts before the snapshot is read, and chunks up to the snapshot's last one are
    // skipped, so output that arrives in between is neither lost nor written twice.
    let shown = -1;
    let gone = false;
    const early: { data: string; seq: number }[] = [];
    const show = (chunk: { data: string; seq: number }) => {
      if (chunk.seq <= shown) return;
      term.write(chunk.data);
      shown = chunk.seq;
    };
    const unlisten = listen<{ id: string; data: string; seq: number }>("session-output", (e) => {
      if (e.payload.id !== id) return;
      if (shown < 0) early.push(e.payload);
      else show(e.payload);
    });
    unlisten
      .then(() => api.sessionOutput(id))
      .catch(() => ({ data: "", seq: 0 }))
      .then((snap) => {
        if (gone) return;
        if (snap.data) term.write(snap.data);
        shown = snap.seq;
        early.splice(0).forEach(show);
      });

    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        // Ignored while the panel is collapsed.
      }
    });
    ro.observe(box);
    if (acceptInput) term.focus();

    return () => {
      gone = true;
      ro.disconnect();
      unlisten.then((f) => f());
      term.dispose();
    };
  });
</script>

<div class="xterm-host" bind:this={host} role="region" aria-label={label}></div>
