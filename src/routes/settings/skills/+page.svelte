<script lang="ts">
  import { onMount } from "svelte";
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import Copy from "phosphor-svelte/lib/Copy";
  import FolderSimple from "phosphor-svelte/lib/FolderSimple";
  import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
  import X from "phosphor-svelte/lib/X";
  import { api, errorText, type SkillRow, type SkillScan } from "$lib/api";
  import CliMark from "$lib/CliMark.svelte";
  import Dialog from "$lib/Dialog.svelte";
  import { ago, shortPath } from "$lib/format";
  import SettingsHead from "$lib/SettingsHead.svelte";
  import { plural, t, tb, type Key } from "$lib/i18n.svelte";
  import { cellFor, copySteps, entryIn, filterSkills, joinPath, skillCounts, skillSources, type Cell, type CopyStep, type SkillFilter } from "$lib/skills";
  import { app, showToast } from "$lib/store.svelte";

  let scan = $state<SkillScan | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let scanning = $state(false);
  let filter = $state<SkillFilter>("all");
  let query = $state("");
  let detailName = $state<string | null>(null);
  let detailOpen = $state(false);
  let sourceRoot = $state("");

  // Keys, not text: t() runs in the template, so the titles and filters follow a language change.
  const STEP_TITLE: Record<CopyStep["kind"], Key> = {
    copy: "work.skills.stepCopy",
    replace: "work.skills.stepReplace",
    link: "work.skills.stepLink",
    nested: "work.skills.stepNested",
  };

  const FILTERS: { id: SkillFilter; label: Key }[] = [
    { id: "all", label: "work.skills.filterAll" },
    { id: "different", label: "work.skills.filterDifferent" },
    { id: "problems", label: "work.skills.filterProblems" },
  ];

  const counts = $derived(scan ? skillCounts(scan.skills) : { all: 0, different: 0, problems: 0 });
  const rows = $derived(scan ? filterSkills(scan.skills, filter, query) : []);
  const folderCount = $derived(scan?.roots.filter((r) => r.exists).length ?? 0);
  const summary = $derived(
    [
      t("work.skills.skillsInFolders", {
        skills: plural(counts.all, "work.skills.skillsOne", "work.skills.skillsMany"),
        folders: plural(folderCount, "work.skills.foldersOne", "work.skills.foldersMany"),
      }),
      plural(counts.different, "work.skills.differsOne", "work.skills.differsMany"),
      plural(counts.problems, "work.skills.problemsOne", "work.skills.problemsMany"),
    ].join(" · "),
  );
  const emptyText = $derived(
    query.trim()
      ? t("work.skills.noMatch", { query: query.trim() })
      : filter === "different"
        ? t("work.skills.noneDifferent")
        : t("work.skills.noneProblems"),
  );
  // The link target sits in code type between the two halves.
  const linkHint = $derived(t("work.skills.linkHint").split("{path}"));

  const detail = $derived(scan?.skills.find((r) => r.name === detailName) ?? null);
  const sources = $derived(detail ? skillSources(detail) : []);
  const source = $derived(sources.find((e) => e.rootId === sourceRoot) ?? sources[0] ?? null);
  const steps = $derived(scan && detail && source ? copySteps(scan.shell, scan.roots, detail, source) : []);

  onMount(load);

  async function load() {
    if (scanning) return;
    scanning = true;
    try {
      scan = await api.scanSkills();
      loadState = "ready";
    } catch (e) {
      loadError = errorText(e);
      loadState = "error";
    } finally {
      scanning = false;
    }
  }

  function openDetail(row: SkillRow) {
    detailName = row.name;
    sourceRoot = skillSources(row)[0]?.rootId ?? "";
    detailOpen = true;
  }

  function rootLabel(id: string) {
    return scan?.roots.find((r) => r.id === id)?.label ?? id;
  }

  async function copy(command: string, rootId: string) {
    try {
      await navigator.clipboard.writeText(command);
      showToast(t("work.skills.copied", { folder: rootLabel(rootId) }));
    } catch {
      showToast(t("work.skills.copyFailed"));
    }
  }
</script>

<svelte:head><title>{t("work.skills.pageTitle")}</title></svelte:head>

{#snippet cellView(c: Cell)}
  {#if c.kind === "missing"}
    <span class="missing">{t("work.skills.missing")}</span>
  {:else if c.kind === "same"}
    <span class="ok"><CheckCircle size={16} aria-hidden="true" />{c.label}</span>
  {:else if c.kind === "version"}
    <span class="version">
      <span class="chip ro">{t("work.skills.version", { variant: c.variant })}</span>
      {#if c.modifiedAt}<span class="when">{ago(c.modifiedAt, app.now)}</span>{/if}
    </span>
  {:else}
    <span class="chip err">{c.label}</span>
  {/if}
{/snippet}

<main class="main" id="skills-main">
  <SettingsHead>
    <button class="btn secondary" type="button" id="btn-rescan-skills" disabled={scanning} onclick={load}>
      <ArrowClockwise size={16} aria-hidden="true" /><span>{scanning ? t("work.skills.reading") : t("work.rescan")}</span>
    </button>
  </SettingsHead>
  <h2 class="sr-only">{t("work.skills.heading")}</h2>
  <p class="note">{t("work.skills.sub")}</p>

  {#if loadState === "loading"}
    <p class="hint" role="status">{t("work.skills.loading")}</p>
  {:else if loadState === "error" || !scan}
    <div class="state-box" role="alert">
      <h2>{t("work.skills.loadError")}</h2>
      <p>{tb(loadError)}</p>
      <button class="btn secondary" type="button" onclick={load}>{t("work.tryAgain")}</button>
    </div>
  {:else if scan.skills.length === 0}
    <div class="state-box" id="skills-empty">
      <h2>{t("work.skills.empty")}</h2>
      <ul class="paths">
        {#each scan.roots as r (r.id)}
          <li><span class="mono" title={r.path}>{shortPath(r.path)}</span> <span class="meta">{r.exists ? r.label : t("work.skills.rootMissing", { folder: r.label })}</span></li>
        {/each}
      </ul>
      <p>{t("work.skills.emptyHint")}</p>
    </div>
  {:else}
    <div class="toolbar" id="skills-toolbar">
      <p class="summary">{summary}</p>
      <div class="seg" role="group" aria-label={t("work.skills.filterGroup")}>
        {#each FILTERS as f (f.id)}
          <button type="button" aria-pressed={filter === f.id} onclick={() => (filter = f.id)}>{t(f.label)} <span class="count">{counts[f.id]}</span></button>
        {/each}
      </div>
      <label class="input-wrap search">
        <MagnifyingGlass size={16} aria-hidden="true" />
        <input type="search" bind:value={query} aria-label={t("work.skills.search")} placeholder={t("work.skills.searchPlaceholder")} spellcheck="false" />
      </label>
    </div>

    {#if rows.length === 0}
      <p class="hint">{emptyText}</p>
    {:else}
      <div class="table-wrap">
        <table id="skill-matrix" aria-busy={scanning}>
          <caption class="sr-only">{t("work.skills.caption")}</caption>
          <thead>
            <tr>
              <th scope="col" class="skill-col">{t("work.skills.colSkill")}</th>
              {#each scan.roots as r (r.id)}
                <th scope="col" title={r.path}>
                  <span class="root-head">
                    {#if r.cli}<CliMark kind={r.cli} small />{:else}<span class="climark sm" aria-hidden="true"><FolderSimple size={14} /></span>{/if}
                    <b>{r.label}</b>
                  </span>
                  {#if !r.exists}<span class="chip idle">{t("work.skills.folderNotFound")}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each rows as row (row.name)}
              <tr>
                <th scope="row">
                  <button class="skill-name" type="button" onclick={() => openDetail(row)}>{row.name}</button>
                  {#if row.description}<span class="desc" title={row.description}>{row.description}</span>{/if}
                  {#if row.variants > 1}<span class="versions">{t("work.skills.versions", { n: row.variants })}</span>{/if}
                </th>
                {#each scan.roots as r (r.id)}
                  <td>
                    {#if r.exists}{@render cellView(cellFor(row, r.id))}{:else}<span class="sr-only">{t("work.skills.folderNotFound")}</span>{/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    <p class="note">{t("work.skills.note")}</p>
  {/if}
</main>

<Dialog bind:open={detailOpen} labelledby="skill-title" wide>
  {#if scan && detail}
    <div class="d-body">
      <div class="row">
        <h2 id="skill-title" class="grow">{detail.name}</h2>
        <button class="icon-btn" type="button" aria-label={t("work.close")} onclick={() => (detailOpen = false)}><X size={18} aria-hidden="true" /></button>
      </div>
      {#if detail.description}<p class="d-desc">{detail.description}</p>{/if}

      <ul class="where" aria-label={t("work.skills.folders")}>
        {#each scan.roots as r (r.id)}
          {@const e = entryIn(detail, r.id)}
          <li>
            {#if r.cli}<CliMark kind={r.cli} small />{:else}<span class="climark sm" aria-hidden="true"><FolderSimple size={14} /></span>{/if}
            <span class="grow">
              <b>{r.label}</b>
              <span class="mono path" title={e?.path ?? r.path}>{shortPath(e?.path ?? joinPath(scan.shell, r.path, detail.name))}</span>
              {#if e?.linkTarget}<span class="mono path" title={e.linkTarget}>{t("work.skills.linksTo", { path: shortPath(e.linkTarget) })}</span>{/if}
            </span>
            {#if r.exists}{@render cellView(cellFor(detail, r.id))}{:else}<span class="chip idle">{t("work.skills.folderNotFound")}</span>{/if}
          </li>
        {/each}
      </ul>

      {#if sources.length === 0}
        <p class="meta">{t("work.skills.noSource")}</p>
      {:else if steps.length === 0}
        <p class="all-same"><CheckCircle size={16} aria-hidden="true" />{t("work.skills.allSame")}</p>
      {:else}
        <div class="field">
          <label class="label" for="skill-source">{t("work.skills.copyFrom")}</label>
          <select class="select" id="skill-source" bind:value={sourceRoot}>
            {#each sources as s (s.rootId)}
              <option value={s.rootId}>{detail.variants > 1 ? t("work.skills.sourceVersion", { variant: s.variant ?? "", folder: rootLabel(s.rootId) }) : rootLabel(s.rootId)}</option>
            {/each}
          </select>
        </div>
        <div class="steps">
          {#each steps as s (s.rootId)}
            {@const label = rootLabel(s.rootId)}
            <section class="step" aria-labelledby="step-{s.rootId}">
              <h3 id="step-{s.rootId}">{t(STEP_TITLE[s.kind], { folder: label })}</h3>
              {#if s.kind === "nested"}
                <p class="meta">{t("work.skills.nestedHint")}</p>
              {:else if s.kind === "link"}
                <p class="meta">
                  {#if s.target}{linkHint[0]}<span class="mono">{shortPath(s.target)}</span>{linkHint[1]}{:else}{t("work.skills.brokenLinkHint")}{/if}
                </p>
              {:else}
                {#if s.kind === "replace"}<p class="meta">{t("work.skills.replaceHint", { folder: label })}</p>{/if}
                <div class="cmd"><small>{scan.shell === "powershell" ? "PowerShell" : t("work.skills.terminal")}</small><code>{s.command}</code></div>
                <div><button class="btn secondary sm" type="button" onclick={() => copy(s.command, s.rootId)}><Copy size={16} aria-hidden="true" />{t("work.copyCommand")}</button></div>
              {/if}
            </section>
          {/each}
        </div>
      {/if}

      <div class="d-foot">
        <span class="meta grow">{t("work.escToClose")}</span>
        <button class="btn secondary" type="button" onclick={() => (detailOpen = false)}>{t("work.close")}</button>
      </div>
    </div>
  {/if}
</Dialog>

<style>
  /* Text straight on the painting uses ink only (DESIGN.md section 2). */
  .toolbar {
    --ink-2: var(--ink);
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .summary {
    margin: 0;
    font-weight: 700;
    flex: 1 1 260px;
  }
  .seg .count {
    font-weight: 600;
  }
  .search {
    flex: 0 1 300px;
    min-width: 220px;
  }
  .table-wrap {
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--surface);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    min-width: 900px;
  }
  thead th {
    text-align: left;
    font-weight: 700;
    color: var(--ink-2);
    background: var(--surface-2);
    padding: 10px 14px;
    vertical-align: top;
  }
  .skill-col {
    width: 28%;
  }
  .root-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .root-head b {
    color: var(--ink);
    white-space: nowrap;
  }
  thead .chip {
    margin-top: 6px;
  }
  tbody th,
  td {
    padding: 12px 14px;
    border-top: 1px solid var(--line);
    vertical-align: top;
    text-align: left;
  }
  tbody th {
    font-weight: 400;
    max-width: 0;
  }
  .skill-name {
    padding: 0;
    border: 0;
    background: none;
    font-size: 14px;
    font-weight: 700;
    color: var(--ink);
    cursor: pointer;
    text-align: left;
    overflow-wrap: anywhere;
  }
  .skill-name:hover {
    text-decoration: underline;
  }
  .desc,
  .versions,
  .when {
    display: block;
    font-size: 12px;
    color: var(--ink-2);
  }
  .desc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
  .missing {
    color: var(--ink-2);
  }
  .ok,
  .all-same {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .ok :global(svg),
  .all-same :global(svg) {
    color: var(--forest);
  }
  .all-same {
    margin: 0;
  }
  .paths {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .paths .mono {
    font-size: 13px;
  }
  .note {
    max-width: 720px;
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }
  .d-desc {
    margin: -8px 0 0;
    color: var(--ink-2);
    line-height: 1.45;
  }
  .where {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .where li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
  }
  .where li + li {
    border-top: 1px solid var(--line);
  }
  .where b {
    display: block;
    font-size: 14px;
  }
  .where .path {
    display: block;
    font-size: 12px;
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .step {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .step h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 800;
  }
  .step p {
    margin: 0;
  }
  .step code {
    font-size: 13px;
  }
  @media (max-width: 720px) {
    .search {
      flex: 1 1 100%;
    }
  }
</style>
