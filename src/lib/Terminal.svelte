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
  import { Terminal, type IDecoration, type IMarker } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount } from "svelte";
  import { api } from "./api";
  import { loadHistory, suggestFor } from "./command-history";
  import { typedText } from "./command-suggest";
  import { t } from "./i18n.svelte";
  import { osc52Text } from "./osc52";
  import { currentPlatform } from "./platform";
  import { app } from "./store.svelte";
  import { pastedImage, pathForPaste, shiftEnter } from "./term-input";

  // `session` is a session's own terminal; `terminal` a plain shell opened in a tab beside it. `shell` says a plain
  // shell runs here, in `folder`, whose commands it suggests first: always in a tab, and in a blank terminal session.
  // `onmenu` takes the right-click in place of the webview's own menu. `onwake` is called with the terminal's size when
  // someone types into it while it is closed, so it can be started again.
  let {
    id,
    live,
    label,
    kind = "session",
    shell,
    folder = "",
    onmenu,
    onwake,
  }: {
    id: string;
    live: boolean;
    label: string;
    kind?: "session" | "terminal";
    shell?: boolean;
    folder?: string;
    onmenu?: (e: MouseEvent, menu: TermMenu) => void;
    onwake?: (cols: number, rows: number) => Promise<unknown> | void;
  } = $props();
  const plainShell = $derived(shell ?? kind === "terminal");

  let host: HTMLDivElement | undefined = $state();
  // What screen readers hear about a suggested command.
  let spoken = $state("");
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
    const asShell = plainShell;
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
      // Decorations, which draw a suggested command, are still marked proposed in xterm.
      allowProposedApi: true,
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

    // Suggested commands, in shell tabs. The shell's script prints OSC 133;B where typing starts, so the text from there
    // to the cursor is what was typed, and the latest command starting with it shows after the cursor. Right Arrow
    // types the rest. A suggestion shows only once the line reads what the keys sent so far make it, so Right Arrow
    // never adds the rest of a suggestion for older text while an echo is on its way.
    const suggesting = asShell && Boolean(folder);
    const where = folder;
    let prompt: { marker: IMarker; x: number } | null = null;
    // Between a prompt and the Enter that runs its command.
    let atPrompt = false;
    let focused = false;
    let ghost: { marker: IMarker; decoration: IDecoration; x: number; rest: string; full: string } | null = null;
    let lastSpoken = "";
    let speakTimer: ReturnType<typeof setTimeout> | undefined;
    // What the line will read once the shell has echoed every key sent: typed characters add to it. A key whose effect
    // cannot be foreseen (Backspace, Tab, the arrows) sets it to null, and the line is taken as it reads once no key
    // has been sent for QUIET ms.
    const QUIET = 150;
    let expected: string | null = null;
    let lastKeyAt = 0;
    let quietTimer: ReturnType<typeof setTimeout> | undefined;
    const hideGhost = () => {
      ghost?.decoration.dispose();
      ghost?.marker.dispose();
      ghost = null;
    };
    /** Notes a key sent to the shell, and looks again once the keys have stopped for a moment. */
    const sentKeys = (data: string) => {
      hideGhost();
      if (data.includes("\r")) atPrompt = false;
      expected = expected !== null && !/[\x00-\x1f\x7f]/.test(data) ? expected + data : null;
      lastKeyAt = Date.now();
      clearTimeout(quietTimer);
      quietTimer = setTimeout(schedule, QUIET);
    };
    const suggested = (): { rest: string; full: string } | null => {
      if (!acceptInput || !atPrompt || !focused || !prompt || prompt.marker.isDisposed || app.settings?.shellSuggestions === false) return null;
      const buf = term.buffer.active;
      if (buf.type !== "normal") return null;
      const typedNow = typedText(buf, { y: prompt.marker.line, x: prompt.x }, { y: buf.baseY + buf.cursorY, x: buf.cursorX });
      if (typedNow === null) return null;
      if (typedNow !== expected) {
        if (Date.now() - lastKeyAt < QUIET) return null;
        expected = typedNow;
      }
      const full = suggestFor(where, typedNow);
      return full ? { rest: full.slice(typedNow.length), full } : null;
    };
    const showGhost = (s: { rest: string; full: string }) => {
      const x = term.buffer.active.cursorX;
      const chars = [...s.rest];
      const width = Math.min(chars.length, term.cols - x);
      if (width <= 0) return;
      const marker = term.registerMarker(0);
      const decoration = term.registerDecoration({ marker, x, width, layer: "top" });
      if (!decoration) {
        marker.dispose();
        return;
      }
      // One cell per character, like the text xterm draws, and clipped at the right edge.
      decoration.onRender((el) => {
        if (el.childElementCount) return;
        el.classList.add("suggestion");
        el.setAttribute("aria-hidden", "true");
        el.style.lineHeight = el.style.height;
        const cell = `${parseFloat(el.style.width) / width}px`;
        el.replaceChildren(
          ...chars.slice(0, width).map((ch) => {
            const span = document.createElement("span");
            span.textContent = ch;
            span.style.width = cell;
            return span;
          }),
        );
      });
      ghost = { marker, decoration, x, rest: s.rest, full: s.full };
      // Screen readers hear it once it has stayed the same for a moment, not on every key.
      clearTimeout(speakTimer);
      speakTimer = setTimeout(() => {
        if (ghost?.full !== s.full || s.full === lastSpoken) return;
        lastSpoken = s.full;
        spoken = t("terminal.suggested", { command: s.full });
      }, 600);
    };
    const update = () => {
      const s = suggested();
      const buf = term.buffer.active;
      const same = ghost && s && ghost.rest === s.rest && ghost.x === buf.cursorX && ghost.marker.line === buf.baseY + buf.cursorY;
      if (same) return;
      hideGhost();
      if (s) showGhost(s);
    };
    let queued = false;
    const schedule = () => {
      if (queued) return;
      queued = true;
      queueMicrotask(() => {
        queued = false;
        if (!gone) update();
      });
    };
    if (suggesting) {
      loadHistory(where);
      term.parser.registerOscHandler(133, (data) => {
        if (data === "B" || data.startsWith("B;")) {
          prompt?.marker.dispose();
          prompt = { marker: term.registerMarker(0), x: term.buffer.active.cursorX };
          atPrompt = true;
          lastSpoken = "";
          expected = null;
          schedule();
        }
        return true;
      });
      term.onWriteParsed(schedule);
      term.onCursorMove(schedule);
      term.onResize(schedule);
    }

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
      if (ghost && e.key === "ArrowRight" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
        if (e.type === "keydown") {
          e.preventDefault();
          const rest = ghost.rest;
          sentKeys(rest);
          send(rest);
        }
        return false;
      }
      if (e.ctrlKey && !e.altKey && !e.metaKey) {
        const key = e.key.toLowerCase();
        // Ctrl+V lets the browser paste, as in Windows Terminal. The AI CLIs expect that on Windows
        // (Claude Code pastes its own images on Alt+V); elsewhere a session keeps Ctrl+V for the CLI.
        // Ctrl+Shift+V (Linux) and Cmd+V (macOS) are never taken by xterm, so the browser pastes those.
        if (key === "v" && (asShell || windows)) return false;
        // A shell has no copy key of its own: Ctrl+C copies a selection and interrupts without one.
        if (asShell && key === "c" && term.hasSelection()) {
          if (e.type === "keydown") {
            navigator.clipboard?.writeText(term.getSelection()).catch(() => undefined);
            term.clearSelection();
          }
          return false;
        }
      }
      if (e.key === "Enter" && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
        const seq = shiftEnter(asShell ? "terminal" : "session", win32Input);
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
      // Focus reports (ESC [ I and ESC [ O) are not keys.
      if (suggesting && clean && clean !== "\x1b[I" && clean !== "\x1b[O") sentKeys(clean);
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

    // A suggestion shows only in the terminal that takes the keys.
    const onFocusIn = () => {
      focused = true;
      if (suggesting) schedule();
    };
    const onFocusOut = () => {
      focused = false;
      hideGhost();
    };
    box.addEventListener("focusin", onFocusIn);
    box.addEventListener("focusout", onFocusOut);

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
      box.removeEventListener("focusin", onFocusIn);
      box.removeEventListener("focusout", onFocusOut);
      clearTimeout(speakTimer);
      clearTimeout(quietTimer);
      ro.disconnect();
      unlisten.then((f) => f());
      term.dispose();
    };
  });
</script>

<div class="xterm-host" bind:this={host} role="region" aria-label={label}></div>
{#if plainShell}<p class="sr-only" aria-live="polite">{spoken}</p>{/if}
