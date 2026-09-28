<script lang="ts">
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CaretRight from "phosphor-svelte/lib/CaretRight";
  import File from "phosphor-svelte/lib/File";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
  import Prohibit from "phosphor-svelte/lib/Prohibit";
  import { onMount, tick, untrack } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { api, errorText, type FindMatch, type FindResult, type FolderEntry, type GitStatus } from "./api";
  import { folderName } from "./format";
  import { plural, t, tb, type Key } from "./i18n.svelte";
  import { splitPath } from "./workspace.svelte";

  let {
    id,
    cwd,
    status,
    version,
    current,
    onopen,
    onrefresh,
  }: {
    id: string;
    cwd: string;
    status: GitStatus | null;
    /** Goes up when the git status changes; the open folders are read again. */
    version: number;
    /** The file shown in the viewer. */
    current: string | null;
    onopen: (path: string, line?: number) => void;
    onrefresh: () => void;
  } = $props();

  const folder = $derived(folderName(cwd));

  type Dir = { state: "loading" | "ready" | "error"; entries: FolderEntry[]; error: string; truncated: boolean };
  const dirs = new SvelteMap<string, Dir>();
  const open = new SvelteSet<string>();
  let focused = $state("");
  let tree: HTMLDivElement | undefined = $state();

  async function load(dir: string) {
    // A folder read before keeps its rows while it is read again, so the tree does not flicker.
    if (!dirs.has(dir)) dirs.set(dir, { state: "loading", entries: [], error: "", truncated: false });
    try {
      const l = await api.folderList(id, dir);
      dirs.set(dir, { state: "ready", entries: l.entries, error: "", truncated: l.truncated });
    } catch (e) {
      dirs.set(dir, { state: "error", entries: [], error: errorText(e), truncated: false });
    }
  }

  function reload() {
    load("");
    for (const dir of open) if (dirs.has(dir)) load(dir);
  }

  onMount(() => load(""));

  // The status changed: a CLI (or anyone) added, removed or edited files. The first status to
  // arrive only tells what the tree just read.
  let seenVersion = untrack(() => version);
  $effect(() => {
    if (version === seenVersion) return;
    const first = seenVersion === 0;
    seenVersion = version;
    if (!first) untrack(reload);
  });

  function toggle(e: FolderEntry) {
    if (open.has(e.path)) open.delete(e.path);
    else {
      open.add(e.path);
      if (dirs.get(e.path)?.state !== "ready") load(e.path);
    }
  }

  function activate(e: FolderEntry) {
    focused = e.path;
    if (e.dir) toggle(e);
    else onopen(e.path);
  }

  type Row = { entry: FolderEntry; level: number } | { note: "loading" | "error" | "empty" | "truncated"; level: number; dir: string; text: string };
  const rows = $derived.by(() => {
    const out: Row[] = [];
    const walk = (dir: string, level: number) => {
      const d = dirs.get(dir);
      if (!d) return;
      if (dir !== "") {
        if (d.state === "loading") return void out.push({ note: "loading", level, dir, text: t("workspace.files.reading") });
        if (d.state === "error") return void out.push({ note: "error", level, dir, text: tb(d.error) });
        if (d.entries.length === 0) return void out.push({ note: "empty", level, dir, text: t("workspace.files.empty") });
      }
      for (const e of d.entries) {
        out.push({ entry: e, level });
        if (e.dir && open.has(e.path)) walk(e.path, level + 1);
      }
      if (d.truncated) out.push({ note: "truncated", level, dir, text: t("workspace.files.truncated") });
    };
    walk("", 1);
    return out;
  });
  const items = $derived(rows.flatMap((r) => ("entry" in r ? [r.entry] : [])));
  // One row takes Tab (roving tabindex): the last one focused, else the open file, else the first.
  const tabStop = $derived(items.some((x) => x.path === focused) ? focused : (items.find((x) => x.path === current)?.path ?? items[0]?.path ?? ""));

  // Git letters for files, and which folders hold a change somewhere below them.
  const marks = $derived.by(() => {
    const files = new Map<string, string>();
    const holders = new Set<string>();
    for (const c of status?.changes ?? []) {
      files.set(c.path, c.code);
      let p = c.path;
      while (p.includes("/")) {
        p = p.slice(0, p.lastIndexOf("/"));
        holders.add(p);
      }
    }
    return { files, holders };
  });

  async function focusRow(path: string) {
    focused = path;
    await tick();
    for (const el of tree?.querySelectorAll<HTMLElement>("[data-path]") ?? []) {
      if (el.dataset.path === path) {
        el.focus();
        el.scrollIntoView?.({ block: "nearest" });
        return;
      }
    }
  }

  const parentOf = (path: string) => (path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "");

  // The ARIA tree keys: Up and Down move, Right opens or steps in, Left closes or steps out.
  function treeKeys(e: KeyboardEvent) {
    const i = items.findIndex((x) => x.path === tabStop);
    const cur = items[i];
    if (!cur) return;
    let next: FolderEntry | undefined;
    switch (e.key) {
      case "ArrowDown":
        next = items[i + 1];
        break;
      case "ArrowUp":
        next = items[i - 1];
        break;
      case "Home":
        next = items[0];
        break;
      case "End":
        next = items.at(-1);
        break;
      case "ArrowRight":
        if (!cur.dir) break;
        if (!open.has(cur.path)) toggle(cur);
        else if (items[i + 1] && parentOf(items[i + 1].path) === cur.path) next = items[i + 1];
        break;
      case "ArrowLeft":
        if (cur.dir && open.has(cur.path)) toggle(cur);
        else next = items.find((x) => x.path === parentOf(cur.path));
        break;
      case "Enter":
      case " ":
        activate(cur);
        break;
      default:
        return;
    }
    e.preventDefault();
    if (next) focusRow(next.path);
  }

  // Search, 200 ms after the last key; the last results stay while the next search runs.
  let query = $state("");
  let mode = $state<"names" | "contents">("names");
  let found = $state<FindResult | null>(null);
  let searching = $state(false);
  let searchError = $state("");

  $effect(() => {
    const q = query.trim();
    const contents = mode === "contents";
    if (!q) {
      found = null;
      searching = false;
      searchError = "";
      return;
    }
    searching = true;
    let gone = false;
    const timer = setTimeout(async () => {
      try {
        const r = await api.folderFind(id, q, contents);
        if (gone) return;
        found = r;
        searchError = "";
      } catch (err) {
        if (!gone) searchError = errorText(err);
      } finally {
        if (!gone) searching = false;
      }
    }, 200);
    return () => {
      gone = true;
      clearTimeout(timer);
    };
  });

  function pickMode(m: "names" | "contents") {
    if (m === mode) return;
    found = null;
    mode = m;
  }

  // Text matches grouped under their file, in the order the backend found them.
  const groups = $derived.by(() => {
    const out: { path: string; lines: FindMatch[] }[] = [];
    for (const m of found?.matches ?? []) {
      const last = out.at(-1);
      if (last?.path === m.path) last.lines.push(m);
      else out.push({ path: m.path, lines: [m] });
    }
    return out;
  });

  /** `text` cut into plain and matching pieces, ignoring case. */
  function pieces(text: string, q: string): { s: string; hit: boolean }[] {
    const lower = text.toLowerCase();
    const needle = q.toLowerCase();
    const out: { s: string; hit: boolean }[] = [];
    let at = 0;
    while (needle) {
      const i = lower.indexOf(needle, at);
      if (i < 0) break;
      if (i > at) out.push({ s: text.slice(at, i), hit: false });
      out.push({ s: text.slice(i, i + needle.length), hit: true });
      at = i + needle.length;
    }
    if (at < text.length) out.push({ s: text.slice(at), hit: false });
    return out;
  }

  function refresh() {
    reload();
    onrefresh();
  }

  function codeLabel(code: string) {
    return t(`workspace.code.${code}` as Key);
  }

  const root = $derived(dirs.get(""));
</script>

<div class="side-head">
  <h3 class="mono" title={cwd}>{folder}</h3>
  <button class="icon-btn sm" type="button" aria-label={t("workspace.refreshNamed", { what: folder })} title={t("workspace.refresh")} onclick={refresh}>
    <ArrowClockwise size={16} aria-hidden="true" />
  </button>
</div>

<div class="find">
  <div class="input-wrap">
    <MagnifyingGlass size={16} aria-hidden="true" />
    <input
      id="files-find"
      type="search"
      bind:value={query}
      placeholder={t("workspace.files.find")}
      aria-label={t("workspace.files.findLabel", { folder })}
      autocomplete="off"
      spellcheck="false"
      onkeydown={(e) => {
        if (e.key === "Escape" && query) {
          e.preventDefault();
          query = "";
        }
      }}
    />
  </div>
  <div class="seg" role="group" aria-label={t("workspace.files.searchBy")}>
    <button type="button" aria-pressed={mode === "names"} onclick={() => pickMode("names")}>{t("workspace.files.names")}</button>
    <button type="button" aria-pressed={mode === "contents"} onclick={() => pickMode("contents")}>{t("workspace.files.contents")}</button>
  </div>
</div>

{#if query.trim()}
  {@const q = query.trim()}
  <div class="results" id="files-results">
    <p class="sr-only" role="status">
      {searching ? t("workspace.files.searching") : found ? plural(found.matches.length, "workspace.files.foundOne", "workspace.files.foundMany") : ""}
    </p>
    {#if searchError}
      <p class="err-text" role="alert">{t("workspace.files.searchFailed")}: {tb(searchError)}</p>
    {:else if !found}
      <p class="side-note">{t("workspace.files.searching")}</p>
    {:else if found.matches.length === 0}
      <p class="side-note">{mode === "names" ? t("workspace.files.noName", { query: q }) : t("workspace.files.noText", { query: q })}</p>
    {:else if mode === "names"}
      <ul class="hits" aria-label={t("workspace.files.results")}>
        {#each found.matches as m (m.path)}
          {@const p = splitPath(m.path)}
          <li>
            <button class="hit" type="button" class:current={m.path === current} title={m.path} onclick={() => onopen(m.path)}>
              <File size={16} aria-hidden="true" />
              <span class="grow">
                <span class="name mono">{#each pieces(p.name, q) as piece, i (i)}{#if piece.hit}<mark>{piece.s}</mark>{:else}{piece.s}{/if}{/each}</span>
                {#if p.dir}<span class="dir mono">{p.dir}</span>{/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <ul class="hits" aria-label={t("workspace.files.results")}>
        {#each groups as g (g.path)}
          {@const p = splitPath(g.path)}
          <li class="hit-group">
            <div class="hit-file" title={g.path}>
              <File size={16} aria-hidden="true" />
              <b class="name mono">{p.name}</b>
              {#if p.dir}<span class="dir mono">{p.dir}</span>{/if}
            </div>
            <ul>
              {#each g.lines as m (m.line)}
                <li>
                  <button
                    class="hit line"
                    type="button"
                    aria-label={t("workspace.files.lineNamed", { path: m.path, line: m.line ?? 0, text: m.text ?? "" })}
                    onclick={() => onopen(m.path, m.line ?? undefined)}
                  >
                    <span class="ln mono" aria-hidden="true">{m.line}</span>
                    <span class="tx mono" aria-hidden="true">{#each pieces(m.text ?? "", q) as piece, i (i)}{#if piece.hit}<mark>{piece.s}</mark>{:else}{piece.s}{/if}{/each}</span>
                  </button>
                </li>
              {/each}
            </ul>
          </li>
        {/each}
      </ul>
    {/if}
    {#if found?.truncated}<p class="side-note">{t("workspace.files.stoppedEarly")}</p>{/if}
  </div>
{:else if !root || (root.state === "loading" && root.entries.length === 0)}
  <p class="side-note" role="status">{t("workspace.files.loading", { folder })}</p>
{:else if root.state === "error"}
  <div class="side-state" role="alert">
    <b>{t("workspace.files.loadFailed", { folder })}</b>
    <p>{tb(root.error)}</p>
    <button class="btn secondary sm" type="button" onclick={() => load("")}>{t("sessions.tryAgain")}</button>
  </div>
{:else if root.entries.length === 0}
  <p class="side-note">{t("workspace.files.empty")}</p>
{:else}
  <div class="tree" id="files-tree" role="tree" aria-label={t("workspace.files.tree", { folder })} bind:this={tree}>
    {#each rows as row ("entry" in row ? row.entry.path : `${row.dir}:${row.note}`)}
      {#if "entry" in row}
        {@const e = row.entry}
        {@const code = e.dir ? undefined : marks.files.get(e.path)}
        {@const holds = e.dir && marks.holders.has(e.path)}
        <div
          class="node"
          class:current={e.path === current}
          role="treeitem"
          aria-level={row.level}
          aria-expanded={e.dir ? open.has(e.path) : undefined}
          aria-selected={e.path === current}
          tabindex={e.path === tabStop ? 0 : -1}
          data-path={e.path}
          style:--level={row.level}
          title={[e.path, e.ignored ? t("workspace.files.ignored") : "", code ? codeLabel(code) : ""].filter(Boolean).join(" · ")}
          onclick={() => activate(e)}
          onkeydown={treeKeys}
          onfocus={() => (focused = e.path)}
        >
          <span class="caret" class:open={open.has(e.path)} aria-hidden="true">{#if e.dir}<CaretRight size={12} weight="bold" />{/if}</span>
          {#if e.dir}<FolderSimple size={16} aria-hidden="true" />{:else}<File size={16} aria-hidden="true" />{/if}
          <span class="name mono" class:ignored={e.ignored}>{e.name}</span>
          {#if e.ignored}
            <Prohibit size={14} aria-hidden="true" /><span class="sr-only">, {t("workspace.files.ignored")}</span>
          {/if}
          {#if code}
            <span class="code mono" aria-hidden="true">{code}</span><span class="sr-only">, {codeLabel(code)}</span>
          {:else if holds}
            <span class="holds" aria-hidden="true"></span><span class="sr-only">, {t("workspace.files.hasChanges")}</span>
          {/if}
        </div>
      {:else}
        <div class="node note" class:err-line={row.note === "error"} role="treeitem" aria-level={row.level} aria-disabled="true" aria-selected="false" tabindex="-1" style:--level={row.level}>
          {row.text}
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .find {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .find input::-webkit-search-cancel-button {
    cursor: pointer;
  }
  .tree,
  .results {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .node {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 2px 8px 2px calc(4px + (var(--level) - 1) * 14px);
    border-radius: var(--r-sm);
    cursor: pointer;
    font-size: 12px;
    transition: background-color 0.12s ease-out;
  }
  .node:hover {
    background: var(--surface-2);
  }
  .node.current {
    background: var(--surface-2);
    font-weight: 700;
  }
  .node:focus-visible {
    outline-offset: -2px;
  }
  .node.note {
    cursor: default;
    padding-left: calc(24px + (var(--level) - 1) * 14px);
    color: var(--ink-2);
  }
  .node.note:hover {
    background: none;
  }
  .node.err-line {
    color: var(--st-err);
  }
  .caret {
    display: grid;
    width: 12px;
    flex: none;
    transition: rotate 0.12s ease-out;
  }
  .caret.open {
    rotate: 90deg;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Ignored entries are told apart by shape, since secondary text on this plate is ink too. */
  .name.ignored {
    font-style: italic;
  }
  .code {
    margin-left: auto;
    flex: none;
    font-weight: 600;
  }
  .holds {
    margin-left: auto;
    flex: none;
    width: 6px;
    height: 6px;
    background: var(--ink);
  }
  .node :global(svg) {
    flex: none;
  }
  .hits,
  .hits ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .hit {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
    font-size: 12px;
  }
  .hit:hover,
  .hit.current {
    background: var(--surface-2);
  }
  .hit:focus-visible {
    outline-offset: -2px;
  }
  .hit .grow {
    display: flex;
    flex-direction: column;
  }
  .hit .name,
  .hit .dir,
  .hit-file .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir {
    font-size: 12px;
    color: var(--ink-2);
  }
  .hit-group {
    padding: 6px 0 2px;
  }
  .hit-file {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px 2px;
    font-size: 12px;
    min-width: 0;
  }
  .hit-file .name {
    font-weight: 700;
  }
  .hit.line {
    padding-left: 30px;
    min-height: 28px;
  }
  .ln {
    flex: none;
    min-width: 3ch;
    text-align: right;
    color: var(--ink-2);
  }
  .tx {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  mark {
    background: none;
    color: inherit;
    font-weight: 800;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  @media (max-width: 720px) {
    .node,
    .hit {
      min-height: 40px;
    }
  }
</style>
