<script module lang="ts">
  /** What a right-click menu over the terminal can do with it. `paste` types the text in while the terminal runs. */
  export type TermMenu = { selection: string; live: boolean; paste: (text: string) => void };

  // The mounted terminals, by session or shell id, so a menu elsewhere (a tab's) can clear one.
  const clearers = new Map<string, () => void>();

  /** Empties the screen and scrollback of the terminal shown for `id`. */
  export function clearTerminal(id: string) {
    clearers.get(id)?.();
  }

  /** Keys the session screen keeps for its tabs, so the terminal does not send them on. */
  export function tabKey(e: KeyboardEvent): boolean {
    if (!e.ctrlKey || e.altKey || e.metaKey) return false;
    if (!e.shiftKey && (e.key === "PageUp" || e.key === "PageDown")) return true;
    return e.shiftKey && e.key.toLowerCase() === "w";
  }
</script>

<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount } from "svelte";
  import { api } from "./api";
  import { t } from "./i18n.svelte";
  import { osc52Text } from "./osc52";
  import { currentPlatform } from "./platform";
  import { pastedImage, pathForPaste, shiftEnter } from "./term-input";

  // `session` is an AI CLI session; `terminal` a plain shell opened in a tab beside it. `onmenu` takes the right-click
  // in place of the webview's own menu. `onwake` is called with the terminal's size when someone types into it while
  // it is closed, so it can be started again.
  let {
    id,
    live,
    label,
    kind = "session",
    onmenu,
    onwake,
  }: {
    id: string;
    live: boolean;
    label: string;
    kind?: "session" | "terminal";
    onmenu?: (e: MouseEvent, menu: TermMenu) => void;
    onwake?: (cols: number, rows: number) => Promise<unknown> | void;
  } = $props();

  let host: HTMLDivElement | undefined = $state();
  // Read inside the xterm callbacks, which outlive the first render.
  let acceptInput = false;
  // Tells the process its size once it runs, when the terminal opened before it did.
  let fitProcess: (() => void) | null = null;
  $effect(() => {
    acceptInput = live;
    if (live) fitProcess?.();
  });

  /** A key someone typed, or text they pasted: not a lone control key, and not a cursor or function key. */
  const typed = (d: string) => d === "\r" || (!d.startsWith("\x1b") && /[^\x00-\x1f\x7f]/.test(d));

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
    // Read once: the callbacks and the cleanup below can run after the parent has dropped what `id` comes from (a
    // closed shell), and reading the prop then throws. The Session screen keys each terminal by its id anyway.
    const tid = id;
    const io: {
      event: string;
      send: (id: string, data: string) => Promise<unknown>;
      resize: (id: string, cols: number, rows: number) => Promise<void>;
      snapshot: (id: string) => Promise<{ data: string; seq: number }>;
    } =
      kind === "terminal"
        ? { event: "terminal-output", send: api.terminalWrite, resize: api.terminalResize, snapshot: api.terminalOutput }
        : { event: "session-output", send: api.sendInput, resize: api.resizeSession, snapshot: api.sessionOutput };
    // xterm keeps its screen reader strings in one global; the input label is applied when the terminal opens.
    Terminal.strings.promptLabel = t("sessions.terminal.input");
    Terminal.strings.tooMuchOutput = t("sessions.terminal.tooMuchOutput");
    // Links open in the default browser on Ctrl+click (Cmd+click on macOS), as in Windows Terminal and Orca, so a click
    // that focuses the terminal or starts a selection opens nothing. The addon finds plain URLs; OSC 8 links, a URL
    // behind other text, come through xterm's own handler, which passes only http and https.
    const mac = currentPlatform() === "macos";
    const openLink = (e: MouseEvent, uri: string) => {
      if (e.button === 0 && (mac ? e.metaKey : e.ctrlKey)) openUrl(uri).catch(() => undefined);
    };
    const showLinkHint = () => (box.title = t("sessions.terminal.openLink", { key: mac ? "⌘" : "Ctrl" }));
    const hideLinkHint = () => box.removeAttribute("title");
    const term = new Terminal({
      linkHandler: { activate: openLink, hover: showLinkHint, leave: hideLinkHint },
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
    // Set while ConPTY takes win32-input-mode keys, which carry the Shift that plain VT drops.
    let win32Input = false;
    const onWin32Mode = (on: boolean) => (params: (number | number[])[]) => {
      if (params.includes(9001)) win32Input = on;
      return false;
    };
    term.parser.registerCsiHandler({ prefix: "?", final: "h" }, onWin32Mode(true));
    term.parser.registerCsiHandler({ prefix: "?", final: "l" }, onWin32Mode(false));

    // OSC 52 copies, such as Claude Code's over SSH, have no click behind them for the webview's clipboard, so Rust
    // writes them: only the last of a burst, and none from the screen replayed on opening.
    let replaying = false;
    let lastCopy: string | null = null;
    term.parser.registerOscHandler(52, (data) => {
      const text = replaying ? null : osc52Text(data);
      if (text === null) return true;
      if (lastCopy === null) {
        queueMicrotask(() => {
          if (lastCopy !== null) api.writeClipboard(lastCopy).catch(() => undefined);
          lastCopy = null;
        });
      }
      lastCopy = text;
      return true;
    });
    const windows = currentPlatform() === "windows";

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon(openLink, { hover: showLinkHint, leave: hideLinkHint }));
    term.open(box);
    try {
      fit.fit();
    } catch {
      // A hidden host has no size yet; the observer fits it once it is shown.
    }

    // Tab belongs to the CLI while it runs, so Ctrl+Tab and Ctrl+Shift+Tab leave the terminal
    // instead. A closed terminal takes no keys, and Tab moves on as everywhere else.
    term.attachCustomKeyEventHandler((e) => {
      // Switching and closing tabs work from inside a terminal too; the page hears the key.
      if (tabKey(e)) return false;
      if (e.ctrlKey && !e.altKey && !e.metaKey) {
        const key = e.key.toLowerCase();
        // Ctrl+V lets the browser paste, as in Windows Terminal. The AI CLIs expect that on Windows
        // (Claude Code pastes its own images on Alt+V); elsewhere a session keeps Ctrl+V for the CLI.
        // Ctrl+Shift+V (Linux) and Cmd+V (macOS) are never taken by xterm, so the browser pastes those.
        if (key === "v" && (kind === "terminal" || windows)) return false;
        // A shell has no copy key of its own: Ctrl+C copies a selection and interrupts without one.
        if (kind === "terminal" && key === "c" && term.hasSelection()) {
          if (e.type === "keydown") {
            navigator.clipboard?.writeText(term.getSelection()).catch(() => undefined);
            term.clearSelection();
          }
          return false;
        }
      }
      if (e.key === "Enter" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
        const seq = shiftEnter(kind, win32Input);
        if (seq) {
          if (e.type === "keydown") {
            e.preventDefault();
            term.input(seq);
          }
          return false;
        }
      }
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
      chain = chain.then(() => io.send(tid, data)).catch(() => undefined);
    };
    // What is typed while the terminal is closed only wakes it; the process that starts gets the keys after that.
    let waking = false;
    term.onData((d) => {
      if (!acceptInput) {
        if (onwake && !waking && typed(d)) {
          waking = true;
          // A start that failed can be tried again with the next key.
          Promise.resolve(onwake(term.cols, term.rows)).finally(() => (waking = false));
        }
        return;
      }
      const clean = d.replace(CURSOR_REPORT, "");
      if (clean) send(clean);
    });
    term.onResize(({ cols, rows }) => {
      if (acceptInput) io.resize(tid, cols, rows).catch(() => undefined);
    });
    fitProcess = () => io.resize(tid, term.cols, term.rows).catch(() => undefined);
    if (acceptInput) fitProcess();

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
    const unlisten = listen<{ id: string; data: string; seq: number }>(io.event, (e) => {
      if (e.payload.id !== tid) return;
      if (shown < 0) early.push(e.payload);
      else show(e.payload);
    });
    unlisten
      .then(() => io.snapshot(tid))
      .catch(() => ({ data: "", seq: 0 }))
      .then((snap) => {
        if (gone) return;
        if (snap.data) {
          replaying = true;
          term.write(snap.data, () => (replaying = false));
        }
        shown = snap.seq;
        early.splice(0).forEach(show);
      });

    // xterm pastes text only, so a screenshot is saved to a file and its path pasted instead.
    // Runs in the capture phase, before xterm's own listener pastes nothing.
    const onPaste = (e: ClipboardEvent) => {
      const image = pastedImage(e.clipboardData);
      if (!image) return;
      e.preventDefault();
      e.stopPropagation();
      if (!acceptInput) return;
      image
        .arrayBuffer()
        .then((buf) => api.savePastedImage(new Uint8Array(buf), image.type))
        .then((path) => {
          if (!gone) term.paste(pathForPaste(path));
        })
        .catch(() => undefined);
    };
    box.addEventListener("paste", onPaste, true);

    const onContext = (e: MouseEvent) => {
      onmenu?.(e, { selection: term.getSelection(), live: acceptInput, paste: (text) => acceptInput && term.paste(text) });
    };
    box.addEventListener("contextmenu", onContext);
    const clear = () => term.clear();
    clearers.set(tid, clear);

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
      fitProcess = null;
      // A remount for the same id may have taken the slot already.
      if (clearers.get(tid) === clear) clearers.delete(tid);
      box.removeEventListener("paste", onPaste, true);
      box.removeEventListener("contextmenu", onContext);
      ro.disconnect();
      unlisten.then((f) => f());
      term.dispose();
    };
  });
</script>

<div class="xterm-host" bind:this={host} role="region" aria-label={label}></div>
