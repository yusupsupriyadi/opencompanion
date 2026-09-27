<script lang="ts">
  import { page } from "$app/state";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import GearSix from "phosphor-svelte/lib/GearSix";
  import HandPalm from "phosphor-svelte/lib/HandPalm";
  import House from "phosphor-svelte/lib/House";
  import Kanban from "phosphor-svelte/lib/Kanban";
  import Plus from "phosphor-svelte/lib/Plus";
  import PushPin from "phosphor-svelte/lib/PushPin";
  import TerminalIcon from "phosphor-svelte/lib/Terminal";
  import Trash from "phosphor-svelte/lib/Trash";
  import type { SessionView } from "./api";
  import AppLogo from "./AppLogo.svelte";
  import CliMark from "./CliMark.svelte";
  import { CLI_LABEL, STATUS, folderName, isLive } from "./format";
  import { plural, t } from "./i18n.svelte";
  import { app, askDelete, askNewSession } from "./store.svelte";

  // CLIs, Skills, Phone access and the theme live under Settings.
  const NAV = [
    { href: "/", key: "shell.nav.overview", icon: House },
    { href: "/board", key: "shell.nav.board", icon: Kanban },
    { href: "/chat", key: "shell.nav.chat", icon: ChatsCircle },
    { href: "/terminal", key: "shell.nav.terminal", icon: TerminalIcon },
    { href: "/settings", key: "shell.nav.settings", icon: GearSix },
  ] as const;

  const path = $derived(page.url.pathname);
  const currentSession = $derived(path === "/session" ? page.url.searchParams.get("id") : null);

  function active(href: string) {
    if (href === "/") return path === "/" || path === "/session";
    return path.startsWith(href);
  }

  // Windows paths: one folder whatever the case, slash direction or trailing slash.
  const folderKey = (cwd: string) => cwd.replace(/[\\/]+$/, "").replace(/\\/g, "/").toLowerCase();

  // Collapsed folders and pins survive a restart.
  const COLLAPSED_KEY = "air-collapsed-folders";
  const PINNED_FOLDERS_KEY = "air-pinned-folders";
  const PINNED_SESSIONS_KEY = "air-pinned-sessions";
  function readList(key: string): string[] {
    try {
      const v: unknown = JSON.parse(localStorage.getItem(key) ?? "[]");
      return Array.isArray(v) ? v.filter((k): k is string => typeof k === "string") : [];
    } catch {
      return [];
    }
  }
  function saveList(key: string, list: string[]) {
    try {
      localStorage.setItem(key, JSON.stringify(list));
    } catch {
      // Private windows can refuse storage; the change still holds for this session.
    }
  }
  const flip = (list: string[], item: string) => (list.includes(item) ? list.filter((k) => k !== item) : [...list, item]);

  let collapsed = $state(readList(COLLAPSED_KEY));
  let pinnedFolders = $state(readList(PINNED_FOLDERS_KEY)); // folder paths as first pinned
  let pinnedSessions = $state(readList(PINNED_SESSIONS_KEY)); // session ids

  function toggle(key: string) {
    collapsed = flip(collapsed, key);
    saveList(COLLAPSED_KEY, collapsed);
  }
  function pinFolder(g: { key: string; cwd: string; pinned: boolean }) {
    pinnedFolders = g.pinned ? pinnedFolders.filter((f) => folderKey(f) !== g.key) : [...pinnedFolders, g.cwd];
    saveList(PINNED_FOLDERS_KEY, pinnedFolders);
  }
  function pinSession(id: string) {
    pinnedSessions = flip(pinnedSessions, id);
    saveList(PINNED_SESSIONS_KEY, pinnedSessions);
  }

  // Every session is shown, most urgent first: live (waiting on top), then finished, newest first.
  // Pinned folders lead, even empty; the rest follow their most urgent session. Inside a folder,
  // pinned sessions come first.
  const groups = $derived.by(() => {
    const live = app.sessions.filter(isLive);
    live.sort((a, b) => Number(b.status === "waiting") - Number(a.status === "waiting"));
    const ordered = [...live, ...app.sessions.filter((s) => !isLive(s))];
    const pinned = new Set(pinnedSessions);

    const byFolder = new Map<string, { key: string; cwd: string; pinned: boolean; sessions: SessionView[] }>();
    for (const cwd of pinnedFolders) {
      const key = folderKey(cwd);
      if (!byFolder.has(key)) byFolder.set(key, { key, cwd, pinned: true, sessions: [] });
    }
    for (const s of ordered) {
      const key = folderKey(s.cwd);
      const group = byFolder.get(key);
      if (group) group.sessions.push(s);
      else byFolder.set(key, { key, cwd: s.cwd, pinned: false, sessions: [s] });
    }
    for (const g of byFolder.values()) g.sessions.sort((a, b) => Number(pinned.has(b.id)) - Number(pinned.has(a.id)));
    return [...byFolder.values()];
  });
</script>

<aside class="sidebar" aria-label="OpenCompanion">
  <a class="brand" href="/" aria-label={t("shell.sidebar.brand")}><AppLogo /><span class="lbl">OpenCompanion</span></a>
  <nav class="nav" aria-label={t("shell.nav.label")}>
    {#each NAV as n (n.href)}
      <a href={n.href} aria-current={active(n.href) ? "page" : undefined} title={t(n.key)}>
        <n.icon size={18} aria-hidden="true" /><span class="lbl">{t(n.key)}</span>
      </a>
    {/each}
  </nav>
  <div class="side-sessions">
    <div class="side-head">
      <span class="side-label">{t("shell.sidebar.sessions")}</span>
      <button class="icon-btn side-add" type="button" aria-label={t("shell.newSession")} title={t("shell.newSession")} onclick={() => askNewSession()}>
        <Plus size={16} aria-hidden="true" />
      </button>
    </div>
    {#if groups.length}
      <div class="minis">
        {#each groups as g, i (g.key)}
          {@const shut = collapsed.includes(g.key)}
          <div class="folder-group" role="group" aria-labelledby="side-folder-{i}">
            <div class="folder-row" class:pinned={g.pinned}>
              <button
                class="folder-head"
                type="button"
                class:holds-current={shut && g.sessions.some((s) => s.id === currentSession)}
                aria-expanded={!shut}
                aria-controls="side-folder-items-{i}"
                title={g.cwd}
                onclick={() => toggle(g.key)}
              >
                <span class="caret" aria-hidden="true"><CaretDown size={12} /></span>
                <FolderSimple size={16} aria-hidden="true" />
                <span class="mono ellipsis" id="side-folder-{i}">{folderName(g.cwd)}</span>
                {#if shut}
                  {#if g.sessions.some((s) => s.status === "waiting")}
                    <span class="head-wait" title={t("shell.status.waiting")}><HandPalm size={14} aria-hidden="true" /><span class="sr-only">{t("shell.status.waiting")}</span></span>
                  {/if}
                  <span class="count">{g.sessions.length} <span class="sr-only">{plural(g.sessions.length, "shell.sidebar.countOne", "shell.sidebar.countMany")}</span></span>
                {/if}
              </button>
              <div class="row-actions">
                <button
                  class="icon-btn row-btn"
                  type="button"
                  aria-label={t("shell.sidebar.newSessionIn", { folder: folderName(g.cwd) })}
                  title={t("shell.sidebar.newSessionIn", { folder: folderName(g.cwd) })}
                  onclick={() => askNewSession(g.cwd)}
                >
                  <Plus size={14} aria-hidden="true" />
                </button>
                <button
                  class="icon-btn row-btn pin-btn"
                  type="button"
                  aria-pressed={g.pinned}
                  aria-label={t("shell.sidebar.pinFolderNamed", { folder: folderName(g.cwd) })}
                  title={g.pinned ? t("shell.sidebar.unpinFolder") : t("shell.sidebar.pinFolder")}
                  onclick={() => pinFolder(g)}
                >
                  <PushPin size={14} weight={g.pinned ? "fill" : "regular"} aria-hidden="true" />
                </button>
              </div>
            </div>
            <div class="folder-items" id="side-folder-items-{i}" hidden={shut}>
              {#if g.sessions.length === 0}
                <p class="folder-empty">{t("shell.sidebar.noRecent")}</p>
              {/if}
              {#each g.sessions as s (s.id)}
                {@const sessionPinned = pinnedSessions.includes(s.id)}
                <div class="mini-item" class:pinned={sessionPinned} class:current={s.id === currentSession}>
                  <a
                    class="mini"
                    href="/session?id={s.id}"
                    aria-current={s.id === currentSession ? "page" : undefined}
                    title="{s.title} · {CLI_LABEL[s.cli]} · {STATUS[s.status].label}"
                  >
                    <CliMark kind={s.cli} bare />
                    <b class="ellipsis" class:shimmer={s.status === "running"}>{s.title}</b>
                    <span class="sr-only">{CLI_LABEL[s.cli]}, {STATUS[s.status].label}</span>
                  </a>
                  <div class="row-actions">
                    {#if !isLive(s)}
                      <button
                        class="icon-btn row-btn del"
                        type="button"
                        aria-label={t("shell.sidebar.deleteSessionNamed", { title: s.title })}
                        title={t("shell.deleteSession")}
                        onclick={() => askDelete(s)}
                      >
                        <Trash size={14} aria-hidden="true" />
                      </button>
                    {/if}
                    <button
                      class="icon-btn row-btn pin-btn"
                      type="button"
                      aria-pressed={sessionPinned}
                      aria-label={t("shell.sidebar.pinSessionNamed", { title: s.title })}
                      title={sessionPinned ? t("shell.sidebar.unpinSession") : t("shell.sidebar.pinSession")}
                      onclick={() => pinSession(s.id)}
                    >
                      <PushPin size={14} weight={sessionPinned ? "fill" : "regular"} aria-hidden="true" />
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</aside>

<style>
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mini-item,
  .folder-row {
    position: relative;
    --row-bg: var(--surface-2);
  }
  .mini-item.current {
    --row-bg: var(--surface);
  }
  /* The whole row lights up while the pointer or keyboard is on it, buttons included. */
  .mini-item:hover .mini,
  .mini-item:focus-within .mini,
  .folder-row:hover .folder-head,
  .folder-row:focus-within .folder-head {
    background: var(--surface-2);
  }
  .mini-item .mini[aria-current="page"] {
    background: var(--surface);
  }
  /* Navigation stays calm: row buttons show on hover or keyboard focus, laid over the end of
     the row. A pin that is on stays visible and keeps its own room. */
  .row-actions {
    position: absolute;
    top: 50%;
    right: 4px;
    translate: 0 -50%;
    display: flex;
    gap: 2px;
    padding-left: 14px;
    border-radius: 0 var(--r-btn) var(--r-btn) 0;
  }
  .mini-item:hover .row-actions,
  .mini-item:focus-within .row-actions,
  .folder-row:hover .row-actions,
  .folder-row:focus-within .row-actions {
    background: linear-gradient(to right, transparent, var(--row-bg) 14px);
  }
  .row-btn {
    width: 26px;
    height: 26px;
    opacity: 0;
    transition: opacity 0.12s ease-out;
  }
  .row-btn:hover {
    background: var(--surface);
  }
  .mini-item:hover .row-btn,
  .mini-item:focus-within .row-btn,
  .folder-row:hover .row-btn,
  .folder-row:focus-within .row-btn,
  .pinned .pin-btn {
    opacity: 1;
  }
  .pinned > .mini,
  .pinned > .folder-head {
    padding-right: 36px;
  }
  .del:hover {
    color: var(--st-err);
  }
  /* Lines up with the session titles: rows indent 12, then logo 16 and gap 6. */
  .folder-empty {
    margin: 0;
    padding: 5px 12px 5px 34px;
    font-size: 12px;
    color: var(--ink-2);
  }
  .side-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 4px;
  }
  .side-head .side-label {
    padding-bottom: 0;
  }
  /* Right edge matches the row buttons below (4 from the edge). */
  .side-add {
    width: 28px;
    height: 28px;
    margin-right: 4px;
  }
</style>
