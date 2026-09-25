<script lang="ts">
  import { page } from "$app/state";
  import Books from "phosphor-svelte/lib/Books";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import DeviceMobile from "phosphor-svelte/lib/DeviceMobile";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import GearSix from "phosphor-svelte/lib/GearSix";
  import HandPalm from "phosphor-svelte/lib/HandPalm";
  import House from "phosphor-svelte/lib/House";
  import Kanban from "phosphor-svelte/lib/Kanban";
  import Moon from "phosphor-svelte/lib/Moon";
  import Plus from "phosphor-svelte/lib/Plus";
  import PushPin from "phosphor-svelte/lib/PushPin";
  import Sun from "phosphor-svelte/lib/Sun";
  import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
  import Trash from "phosphor-svelte/lib/Trash";
  import type { SessionView } from "./api";
  import CliMark from "./CliMark.svelte";
  import { CLI_LABEL, STATUS, folderName, isLive } from "./format";
  import { app, askDelete, askNewSession, currentTheme, setTheme } from "./store.svelte";

  const NAV = [
    { href: "/", label: "Overview", icon: House },
    { href: "/board", label: "Board", icon: Kanban },
    { href: "/chat", label: "Chat", icon: ChatsCircle },
    { href: "/clis", label: "CLIs", icon: TerminalWindow },
    { href: "/skills", label: "Skills", icon: Books },
    { href: "/settings", label: "Settings", icon: GearSix },
  ];

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

  // Shown: every pinned session plus the six most urgent others (live first, waiting on top,
  // then recent finished). Pinned folders lead, even empty; the rest follow their most urgent
  // session. Inside a folder, pinned sessions come first.
  const groups = $derived.by(() => {
    const live = app.sessions.filter(isLive);
    live.sort((a, b) => Number(b.status === "waiting") - Number(a.status === "waiting"));
    const ordered = [...live, ...app.sessions.filter((s) => !isLive(s))];
    const pinned = new Set(pinnedSessions);
    const shown = new Set([...ordered.filter((s) => pinned.has(s.id)), ...ordered.filter((s) => !pinned.has(s.id)).slice(0, 6)]);

    const byFolder = new Map<string, { key: string; cwd: string; pinned: boolean; sessions: SessionView[] }>();
    for (const cwd of pinnedFolders) {
      const key = folderKey(cwd);
      if (!byFolder.has(key)) byFolder.set(key, { key, cwd, pinned: true, sessions: [] });
    }
    for (const s of ordered) {
      if (!shown.has(s)) continue;
      const key = folderKey(s.cwd);
      const group = byFolder.get(key);
      if (group) group.sessions.push(s);
      else byFolder.set(key, { key, cwd: s.cwd, pinned: false, sessions: [s] });
    }
    for (const g of byFolder.values()) g.sessions.sort((a, b) => Number(pinned.has(b.id)) - Number(pinned.has(a.id)));
    return [...byFolder.values()];
  });

  const phoneOn = $derived(app.companion?.running ?? false);
</script>

<aside class="sidebar" aria-label="AI Remote">
  <a class="brand" href="/" aria-label="AI Remote, Overview"><span class="lbl">AI Remote</span><Cloud size={20} aria-hidden="true" /></a>
  <nav class="nav" aria-label="Main">
    {#each NAV as n (n.href)}
      <a href={n.href} aria-current={active(n.href) ? "page" : undefined} title={n.label}>
        <n.icon size={18} aria-hidden="true" /><span class="lbl">{n.label}</span>
      </a>
    {/each}
  </nav>
  <div class="side-sessions">
    <div class="side-head">
      <span class="side-label">Sessions</span>
      <button class="icon-btn side-add" type="button" aria-label="New session" title="New session" onclick={() => askNewSession()}>
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
                    <span class="head-wait" title="Waiting for you"><HandPalm size={14} aria-hidden="true" /><span class="sr-only">Waiting for you</span></span>
                  {/if}
                  <span class="count">{g.sessions.length} <span class="sr-only">{g.sessions.length === 1 ? "session" : "sessions"}</span></span>
                {/if}
              </button>
              <div class="row-actions">
                <button
                  class="icon-btn row-btn"
                  type="button"
                  aria-label="New session in {folderName(g.cwd)}"
                  title="New session in {folderName(g.cwd)}"
                  onclick={() => askNewSession(g.cwd)}
                >
                  <Plus size={14} aria-hidden="true" />
                </button>
                <button
                  class="icon-btn row-btn pin-btn"
                  type="button"
                  aria-pressed={g.pinned}
                  aria-label="Pin folder: {folderName(g.cwd)}"
                  title={g.pinned ? "Unpin folder" : "Pin folder"}
                  onclick={() => pinFolder(g)}
                >
                  <PushPin size={14} weight={g.pinned ? "fill" : "regular"} aria-hidden="true" />
                </button>
              </div>
            </div>
            <div class="folder-items" id="side-folder-items-{i}" hidden={shut}>
              {#if g.sessions.length === 0}
                <p class="folder-empty">No recent sessions</p>
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
                      <button class="icon-btn row-btn del" type="button" aria-label="Delete session: {s.title}" title="Delete session" onclick={() => askDelete(s)}>
                        <Trash size={14} aria-hidden="true" />
                      </button>
                    {/if}
                    <button
                      class="icon-btn row-btn pin-btn"
                      type="button"
                      aria-pressed={sessionPinned}
                      aria-label="Pin session: {s.title}"
                      title={sessionPinned ? "Unpin session" : "Pin session"}
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
  <div class="side-foot">
    <a class="phone-row" href="/settings#phone" title="Phone access">
      <DeviceMobile size={18} aria-hidden="true" />
      <span class="lbl grow">Phone access</span>
      <span class="chip {phoneOn ? 'run' : 'idle'}">{phoneOn ? "On" : "Off"}</span>
    </a>
    <div class="seg" role="group" aria-label="Theme">
      <button type="button" aria-pressed={currentTheme() === "light"} onclick={() => setTheme("light")}>
        <Sun size={16} aria-hidden="true" /><span class="lbl">Day</span>
      </button>
      <button type="button" aria-pressed={currentTheme() === "dark"} onclick={() => setTheme("dark")}>
        <Moon size={16} aria-hidden="true" /><span class="lbl">Dusk</span>
      </button>
    </div>
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
