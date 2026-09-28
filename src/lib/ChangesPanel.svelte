<script lang="ts">
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import type { GitChange } from "./api";
  import { folderName } from "./format";
  import { plural, t, tb, type Key } from "./i18n.svelte";
  import { splitPath, type GitWatch } from "./workspace.svelte";

  let {
    watch,
    cwd,
    current,
    onopen,
  }: {
    watch: GitWatch;
    cwd: string;
    /** The change shown in the viewer. */
    current: string | null;
    onopen: (change: GitChange) => void;
  } = $props();

  const st = $derived(watch.status);
  const totals = $derived(
    (st?.changes ?? []).reduce((sum, c) => ({ added: sum.added + (c.added ?? 0), removed: sum.removed + (c.removed ?? 0) }), { added: 0, removed: 0 }),
  );
  const kind = (c: GitChange) => t(`workspace.code.${c.code}` as Key);
  const label = (c: GitChange) => {
    const row = t("workspace.changes.rowNamed", { path: c.path, kind: kind(c) });
    return c.added !== null && c.removed !== null ? `${row}, ${t("workspace.changes.linesNamed", { added: c.added, removed: c.removed })}` : row;
  };
</script>

<div class="side-head">
  <h3>{t("workspace.tab.changes")}</h3>
  <button class="icon-btn sm" type="button" aria-label={t("workspace.refreshNamed", { what: t("workspace.tab.changes") })} title={t("workspace.refresh")} onclick={() => watch.refresh()}>
    <ArrowClockwise size={16} aria-hidden="true" />
  </button>
</div>

{#if !st && watch.state === "error"}
  <div class="side-state" role="alert">
    <b>{t("workspace.gitFailed")}</b>
    <p>{tb(watch.error)}</p>
    <button class="btn secondary sm" type="button" onclick={() => watch.refresh()}>{t("sessions.tryAgain")}</button>
  </div>
{:else if !st}
  <p class="side-note" role="status">{t("workspace.gitLoading")}</p>
{:else if !st.repo}
  <div class="side-state">
    <b>{t("workspace.notRepo", { folder: folderName(cwd) })}</b>
    <p>{t("workspace.notRepoBody")}</p>
  </div>
{:else if st.changes.length === 0}
  <div class="side-state">
    <b>{t("workspace.changes.none")}</b>
    <p>{st.head ? t("workspace.changes.noneBody", { head: st.head }) : t("workspace.branch.noCommits")}</p>
  </div>
{:else}
  <p class="summary">
    <b>{plural(st.changes.length, "workspace.changes.filesOne", "workspace.changes.filesMany")}</b>
    <span class="mono">+{totals.added} −{totals.removed}</span>
    {#if st.head}<span>{t("workspace.changes.since", { head: st.head })}</span>{:else}<span>{t("workspace.changes.noCommits")}</span>{/if}
  </p>
  <ul class="changes" id="changes-list" aria-label={t("workspace.changes.list")}>
    {#each st.changes as c (c.path)}
      {@const p = splitPath(c.path)}
      <li>
        <button class="change" class:current={c.path === current} type="button" aria-current={c.path === current ? "true" : undefined} aria-label={label(c)} title={`${c.path} · ${kind(c)}`} onclick={() => onopen(c)}>
          <span class="code mono" aria-hidden="true">{c.code}</span>
          <span class="grow">
            <span class="name mono">{p.name}</span>
            {#if c.oldPath}
              <span class="dir mono">{t("workspace.changes.from", { path: c.oldPath })}</span>
            {:else if p.dir}
              <span class="dir mono">{p.dir}</span>
            {/if}
          </span>
          {#if c.added !== null && c.removed !== null}
            <span class="counts mono" aria-hidden="true">+{c.added} −{c.removed}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
  {#if st.truncated}<p class="side-note">{t("workspace.changes.truncated")}</p>{/if}
{/if}
{#if st && watch.state === "error"}
  <p class="err-text" role="alert">{t("workspace.gitFailed")}: {tb(watch.error)}</p>
{/if}

<style>
  .summary {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
    margin: 0;
    font-size: 13px;
  }
  .summary .mono {
    font-size: 12px;
  }
  .changes {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }
  .change {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    min-height: 36px;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.12s ease-out;
  }
  .change:hover,
  .change.current {
    background: var(--surface-2);
  }
  .change:focus-visible {
    outline-offset: -2px;
  }
  .change .grow {
    display: flex;
    flex-direction: column;
  }
  .code {
    flex: none;
    width: 1.5ch;
    font-size: 12px;
    font-weight: 700;
    line-height: 18px;
  }
  .name {
    font-size: 12px;
    font-weight: 700;
    line-height: 18px;
  }
  .name,
  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir,
  .counts {
    font-size: 12px;
    color: var(--ink-2);
  }
  .counts {
    flex: none;
    line-height: 18px;
  }
  @media (max-width: 720px) {
    .change {
      min-height: 44px;
    }
  }
</style>
