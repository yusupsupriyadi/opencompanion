<script lang="ts">
  import { page } from "$app/state";
  import CaretDown from "phosphor-svelte/lib/CaretDown";
  import CaretRight from "phosphor-svelte/lib/CaretRight";
  import ChatsCircle from "phosphor-svelte/lib/ChatsCircle";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import GearSix from "phosphor-svelte/lib/GearSix";
  import HandPalm from "phosphor-svelte/lib/HandPalm";
  import House from "phosphor-svelte/lib/House";
  import Plus from "phosphor-svelte/lib/Plus";
  import PushPin from "phosphor-svelte/lib/PushPin";
  import PushPinSlash from "phosphor-svelte/lib/PushPinSlash";
  import type { SessionView } from "./api";
  import AppLogo from "./AppLogo.svelte";
  import CliMark from "./CliMark.svelte";
  import { folderEntries, menu, openMenu, sessionMenu } from "./context-menu.svelte";
  import { CLI_LABEL, STATUS, folderName, isLive } from "./format";
  import { plural, t } from "./i18n.svelte";
  import { app, askNewSession } from "./store.svelte";

  // CLIs, Skills, Phone access and the theme live under Settings.
  const NAV = [
    { href: "/", key: "shell.nav.overview", icon: House },
    { href: "/chat", key: "shell.nav.chat", icon: ChatsCircle },
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

  type Group = { key: string; cwd: string; pinned: boolean; sessions: SessionView[] };

  // Rows carry no buttons: their actions live in the right-click menu (Shift+F10 or the Menu key).
  const menuFor = (key: string) => menu.open && menu.key === key;
  function sessionContext(e: MouseEvent, s: SessionView) {
    const pinned = pinnedSessions.includes(s.id);
    openMenu(e, `side:${s.id}`, t("shell.menu.sessionLabel", { title: s.title }), sessionMenu(s, { pinned, toggle: () => pinSession(s.id) }));
  }
  function folderContext(e: MouseEvent, g: Group) {
    const shut = collapsed.includes(g.key);
    const folder = folderName(g.cwd);
    openMenu(e, `folder:${g.key}`, t("shell.menu.folderLabel", { folder }), [
      { label: t("shell.sidebar.newSessionIn", { folder }), icon: Plus, action: () => askNewSession(g.cwd) },
      { label: t(g.pinned ? "shell.sidebar.unpinFolder" : "shell.sidebar.pinFolder"), icon: g.pinned ? PushPinSlash : PushPin, action: () => pinFolder(g) },
      { label: t(shut ? "shell.menu.expand" : "shell.menu.collapse"), icon: shut ? CaretDown : CaretRight, action: () => toggle(g.key) },
      null,
      ...folderEntries(g.cwd),
    ]);
  }

  // Every session is shown, most urgent first: live (waiting on top), then finished, newest first.
  // Pinned folders lead, even empty; the rest follow their most urgent session. Inside a folder,
  // pinned sessions come first.
  const groups = $derived.by(() => {
    const live = app.sessions.filter(isLive);
    live.sort((a, b) => Number(b.status === "waiting") - Number(a.status === "waiting"));
    const ordered = [...live, ...app.sessions.filter((s) => !isLive(s))];
    const pinned = new Set(pinnedSessions);

    const byFolder = new Map<string, Group>();
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
            <!-- The button inside takes the keys: Shift+F10 or the Menu key on it bubbles up here. -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="folder-row" class:menu-open={menuFor(`folder:${g.key}`)} oncontextmenu={(e) => folderContext(e, g)}>
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
                {#if g.pinned}
                  <span class="pin-mark"><PushPin size={14} weight="fill" aria-hidden="true" /><span class="sr-only">{t("shell.sidebar.pinned")}</span></span>
                {/if}
                {#if shut}
                  {#if g.sessions.some((s) => s.status === "waiting")}
                    <span class="head-wait" title={t("shell.status.waiting")}><HandPalm size={14} aria-hidden="true" /><span class="sr-only">{t("shell.status.waiting")}</span></span>
                  {/if}
                  <span class="count">{g.sessions.length} <span class="sr-only">{plural(g.sessions.length, "shell.sidebar.countOne", "shell.sidebar.countMany")}</span></span>
                {/if}
              </button>
            </div>
            <div class="folder-items" id="side-folder-items-{i}" hidden={shut}>
              {#if g.sessions.length === 0}
                <p class="folder-empty">{t("shell.sidebar.noRecent")}</p>
              {/if}
              {#each g.sessions as s (s.id)}
                {@const sessionPinned = pinnedSessions.includes(s.id)}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="mini-item"
                  class:current={s.id === currentSession}
                  class:menu-open={menuFor(`side:${s.id}`)}
                  oncontextmenu={(e) => sessionContext(e, s)}
                >
                  <a
                    class="mini"
                    href="/session?id={s.id}"
                    aria-current={s.id === currentSession ? "page" : undefined}
                    title="{s.title} · {CLI_LABEL[s.cli]} · {STATUS[s.status].label}"
                  >
                    <CliMark kind={s.cli} bare />
                    <b class="ellipsis" class:shimmer={s.status === "running"}>{s.title}</b>
                    {#if sessionPinned}<span class="pin-mark"><PushPin size={14} weight="fill" aria-hidden="true" /></span>{/if}
                    <span class="sr-only">{CLI_LABEL[s.cli]}, {STATUS[s.status].label}{#if sessionPinned}, {t("shell.sidebar.pinned")}{/if}</span>
                  </a>
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
  /* The whole row lights up while the pointer or keyboard is on it, and while its right-click
     menu is open. */
  .mini-item:hover .mini,
  .mini-item:focus-within .mini,
  .mini-item.menu-open .mini,
  .folder-row:hover .folder-head,
  .folder-row:focus-within .folder-head,
  .folder-row.menu-open .folder-head {
    background: var(--surface-2);
  }
  .mini-item .mini[aria-current="page"] {
    background: var(--surface);
  }
  /* A pin is a state, not a button: pinning and unpinning are in the menu. */
  .pin-mark {
    display: grid;
    flex: none;
    color: var(--ink-2);
  }
  .mini .pin-mark {
    margin-left: auto;
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
  .side-add {
    width: 28px;
    height: 28px;
    margin-right: 4px;
  }
</style>
