<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import FolderSimplePlus from "phosphor-svelte/lib/FolderSimplePlus";
  import { api, errorText, type ProjectFolder } from "./api";
  import { shortPath } from "./format";
  import { t } from "./i18n.svelte";

  /** `@` in the composer lists project folders; picking one writes its path into the message.
   * The phone passes its own `list` and no Browse: a folder dialog would open on the computer. */
  let {
    textarea,
    list = api.projectFolders,
    canBrowse = true,
  }: { textarea: HTMLTextAreaElement | undefined; list?: () => Promise<ProjectFolder[]>; canBrowse?: boolean } = $props();

  const LIST_ID = "folder-mention-list";
  const MAX = 8;

  let folders = $state<ProjectFolder[]>([]);
  let loadState = $state<"idle" | "loading" | "ready" | "error">("idle");
  let loadError = $state("");
  // The `@word` before the caret: where its `@` sits and what follows it.
  let token = $state<{ start: number; query: string } | null>(null);
  // Escape closes the list for this `@` until another one is typed.
  let dismissed = $state<number | null>(null);
  let active = $state(0);

  const open = $derived(token !== null && dismissed !== token.start);

  const squash = (s: string) => s.toLowerCase().replace(/[-_\s]/g, "");

  // Name starts with the query first, then name contains it, then path contains it.
  const matches = $derived.by(() => {
    if (!token) return [];
    const q = squash(token.query);
    const raw = token.query.toLowerCase();
    const rank = (f: ProjectFolder) => {
      const name = squash(f.name);
      if (!q || name.startsWith(q)) return 0;
      if (name.includes(q)) return 1;
      return f.path.toLowerCase().includes(raw) ? 2 : -1;
    };
    return folders
      .map((f) => ({ f, r: rank(f) }))
      .filter((x) => x.r >= 0)
      .sort((a, b) => a.r - b.r)
      .slice(0, MAX)
      .map((x) => x.f);
  });

  // The folders, then "Browse" last.
  const count = $derived(matches.length + (canBrowse ? 1 : 0));
  const optionId = (i: number) => `folder-mention-${i}`;

  function readToken(ta: HTMLTextAreaElement) {
    if (ta.selectionStart !== ta.selectionEnd) {
      token = null;
      return;
    }
    const before = ta.value.slice(0, ta.selectionStart);
    const m = before.match(/(?:^|\s)@([^\s@"]*)$/);
    const next = m ? { start: before.length - m[1].length - 1, query: m[1] } : null;
    if (next?.start !== token?.start || next?.query !== token?.query) active = 0;
    if (!next || next.start !== dismissed) dismissed = null;
    token = next;
  }

  $effect(() => {
    const ta = textarea;
    if (!ta) return;
    const read = () => readToken(ta);
    const moved = (e: KeyboardEvent) => {
      if (!["ArrowUp", "ArrowDown", "Enter", "Tab", "Escape"].includes(e.key)) read();
    };
    const close = () => (token = null);
    ta.addEventListener("input", read);
    ta.addEventListener("click", read);
    ta.addEventListener("keyup", moved);
    ta.addEventListener("blur", close);
    return () => {
      ta.removeEventListener("input", read);
      ta.removeEventListener("click", read);
      ta.removeEventListener("keyup", moved);
      ta.removeEventListener("blur", close);
    };
  });

  // The text box keeps focus; screen readers follow the highlighted option through it.
  $effect(() => {
    const ta = textarea;
    if (!ta) return;
    ta.setAttribute("aria-autocomplete", "list");
    if (open && count > 0) {
      ta.setAttribute("aria-controls", LIST_ID);
      ta.setAttribute("aria-activedescendant", optionId(active));
    } else {
      ta.removeAttribute("aria-controls");
      ta.removeAttribute("aria-activedescendant");
    }
  });

  async function load() {
    loadState = "loading";
    try {
      folders = await list();
      loadState = "ready";
    } catch (e) {
      loadState = "error";
      loadError = errorText(e);
    }
  }

  $effect(() => {
    if (open && loadState === "idle") load();
  });

  function insert(path: string, at: { start: number; query: string }) {
    const ta = textarea;
    if (!ta) return;
    const text = /\s/.test(path) ? `@"${path}" ` : `@${path} `;
    const end = at.start + 1 + at.query.length;
    ta.value = ta.value.slice(0, at.start) + text + ta.value.slice(end);
    const caret = at.start + text.length;
    ta.setSelectionRange(caret, caret);
    // Lets the composer's bind:value see the new text.
    ta.dispatchEvent(new Event("input", { bubbles: true }));
    token = null;
  }

  async function browse(at: { start: number; query: string }) {
    const picked = await openDialog({ directory: true, multiple: false, title: t("chat.mention.dialogTitle") });
    textarea?.focus();
    if (typeof picked === "string") insert(picked, at);
  }

  function choose(i: number) {
    if (!token) return;
    if (i < matches.length) insert(matches[i].path, token);
    else browse(token);
  }

  /** The list's keys. True when the key was used, so the composer does not also send. */
  export function keydown(e: KeyboardEvent): boolean {
    if (!open || !token || e.isComposing) return false;
    if (count === 0 && e.key !== "Escape") return false;
    if (e.key === "ArrowDown") active = (active + 1) % count;
    else if (e.key === "ArrowUp") active = (active - 1 + count) % count;
    else if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") choose(active);
    else if (e.key === "Escape") dismissed = token.start;
    else return false;
    e.preventDefault();
    return true;
  }
</script>

{#if open && token}
  <div class="mention" id="folder-mention">
    {#if loadState === "loading" || loadState === "idle"}
      <p class="meta" role="status">{t("chat.mention.loading")}</p>
    {:else if loadState === "error"}
      <p class="err-text" role="alert">{t("chat.mention.loadFailed", { error: loadError })}</p>
    {:else if matches.length === 0}
      <p class="meta" role="status">{t(canBrowse ? "chat.mention.noMatch" : "chat.mention.noMatchType", { query: token.query })}</p>
    {:else}
      <p class="sr-only" role="status">{t("chat.mention.count", { n: matches.length })}</p>
    {/if}
    {#if count > 0}
    <!-- Keys go through the text box (aria-activedescendant); a click writes the path. -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <ul class="options" role="listbox" id={LIST_ID} aria-label={t("chat.mention.listLabel")}>
      {#each matches as f, i (f.path)}
        <li
          role="option"
          id={optionId(i)}
          aria-selected={i === active}
          class:active={i === active}
          title={f.path}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => choose(i)}
        >
          <FolderSimple size={16} aria-hidden="true" />
          <b>{f.name}</b>
          <span class="mono path">{shortPath(f.path)}</span>
        </li>
      {/each}
      {#if canBrowse}
        <li
          role="option"
          id={optionId(matches.length)}
          aria-selected={active === matches.length}
          class:active={active === matches.length}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => choose(matches.length)}
        >
          <FolderSimplePlus size={16} aria-hidden="true" />
          <b>{t("chat.mention.browse")}</b>
        </li>
      {/if}
    </ul>
    <p class="keys">{t("chat.mention.keys")}</p>
    {/if}
  </div>
{/if}

<style>
  .mention {
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: calc(100% + 6px);
    max-width: 560px;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px;
    border-radius: var(--r-md);
    background: var(--surface);
    border: 1px solid var(--line-strong);
  }
  .mention > p {
    margin: 0;
    padding: 4px 8px;
    font-size: 12px;
  }
  .mention > .err-text {
    overflow-wrap: anywhere;
  }
  .options {
    list-style: none;
    margin: 0;
    padding: 0;
    /* The phone lowers it, so the list fits above the composer with the on-screen keyboard up. */
    max-height: var(--mention-max, 300px);
    overflow-y: auto;
  }
  .options li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 38px;
    padding: 6px 8px;
    border-radius: var(--r-btn);
    cursor: pointer;
    color: var(--ink);
  }
  .options li :global(svg) {
    color: var(--ink-2);
  }
  .options li:hover,
  .options li.active {
    background: var(--surface-2);
  }
  .options b {
    font-size: 13px;
    font-weight: 700;
    white-space: nowrap;
  }
  .path {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .keys {
    color: var(--ink-2);
    border-top: 1px solid var(--line);
  }
  @media (max-width: 720px) {
    .mention {
      left: 0;
      right: 0;
    }
    .options li {
      min-height: 44px;
    }
    .keys {
      display: none;
    }
  }
</style>
