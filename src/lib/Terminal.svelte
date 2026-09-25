<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount } from "svelte";
  import { api } from "./api";

  let { id, initial, live, label }: { id: string; initial: string; live: boolean; label: string } = $props();

  let host: HTMLDivElement | undefined = $state();
  // Read inside the xterm callbacks, which outlive the first render.
  let acceptInput = false;
  $effect(() => {
    acceptInput = live;
  });

  // Rust answers ConPTY's cursor query itself (docs/spike/M0-results.md), so xterm's own
  // answer is dropped instead of being typed into the CLI.
  const CURSOR_REPORT = /\x1b\[\d+;\d+R/g;

  onMount(() => {
    acceptInput = live;
    if (!host) return;
    const term = new Terminal({
      fontFamily: '"IBM Plex Mono", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      scrollback: 8000,
      allowTransparency: true,
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
    term.open(host);
    try {
      fit.fit();
    } catch {
      // A hidden host has no size yet; the observer fits it once it is shown.
    }
    if (initial) term.write(initial);

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

    const unlisten = listen<{ id: string; data: string }>("session-output", (e) => {
      if (e.payload.id === id) term.write(e.payload.data);
    });
    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        // Ignored while the panel is collapsed.
      }
    });
    ro.observe(host);
    if (acceptInput) term.focus();

    return () => {
      ro.disconnect();
      unlisten.then((f) => f());
      term.dispose();
    };
  });
</script>

<div class="xterm-host" bind:this={host} role="region" aria-label={label}></div>
