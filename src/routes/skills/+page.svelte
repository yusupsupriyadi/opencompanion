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

  const STEP_TITLE: Record<CopyStep["kind"], (label: string) => string> = {
    copy: (l) => `Add to ${l}`,
    replace: (l) => `Replace in ${l}`,
    link: (l) => `${l} uses a link`,
    nested: (l) => `${l} holds a link`,
  };

  const FILTERS: { id: SkillFilter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "different", label: "Different" },
    { id: "problems", label: "Problems" },
  ];

  const counts = $derived(scan ? skillCounts(scan.skills) : { all: 0, different: 0, problems: 0 });
  const rows = $derived(scan ? filterSkills(scan.skills, filter, query) : []);
  const folderCount = $derived(scan?.roots.filter((r) => r.exists).length ?? 0);
  const summary = $derived(
    [
      `${counts.all} ${counts.all === 1 ? "skill" : "skills"} in ${folderCount} ${folderCount === 1 ? "folder" : "folders"}`,
      `${counts.different} ${counts.different === 1 ? "differs" : "differ"}`,
      `${counts.problems} ${counts.problems === 1 ? "has a problem" : "have a problem"}`,
    ].join(" · "),
  );
  const emptyText = $derived(
    query.trim()
      ? `No skill matches "${query.trim()}".`
      : filter === "different"
        ? "No skill differs between folders."
        : "No folder has a problem.",
  );

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
      showToast(`Copied the command for ${rootLabel(rootId)}.`);
    } catch {
      showToast("Could not copy. Select the command and copy it by hand.");
    }
  }
</script>

<svelte:head><title>Skills · AI Remote</title></svelte:head>

{#snippet cellView(c: Cell)}
  {#if c.kind === "missing"}
    <span class="missing">Missing</span>
  {:else if c.kind === "same"}
    <span class="ok"><CheckCircle size={16} aria-hidden="true" />{c.label}</span>
  {:else if c.kind === "version"}
    <span class="version">
      <span class="chip ro">Version {c.variant}</span>
      {#if c.modifiedAt}<span class="when">{ago(c.modifiedAt, app.now)}</span>{/if}
    </span>
  {:else}
    <span class="chip err">{c.label}</span>
  {/if}
{/snippet}

<main class="main" id="skills-main">
  <header class="page-head">
    <div class="grow">
      <h1>Skills</h1>
      <p class="sub">Skill folders in your home directory, compared by content. AI Remote reads these folders and never changes them.</p>
    </div>
    <button class="btn secondary" type="button" id="btn-rescan-skills" disabled={scanning} onclick={load}>
      <ArrowClockwise size={16} aria-hidden="true" /><span>{scanning ? "Reading…" : "Rescan"}</span>
    </button>
  </header>

  {#if loadState === "loading"}
    <p class="hint" role="status">Reading skill folders…</p>
  {:else if loadState === "error" || !scan}
    <div class="state-box" role="alert">
      <h2>Skill folders could not be read</h2>
      <p>{loadError}</p>
      <button class="btn secondary" type="button" onclick={load}>Try again</button>
    </div>
  {:else if scan.skills.length === 0}
    <div class="state-box" id="skills-empty">
      <h2>No skills in these folders yet</h2>
      <ul class="paths">
        {#each scan.roots as r (r.id)}
          <li><span class="mono" title={r.path}>{shortPath(r.path)}</span> <span class="meta">{r.label}{r.exists ? "" : ", folder not found"}</span></li>
        {/each}
      </ul>
      <p>Skills you add to any of them show up here after Rescan.</p>
    </div>
  {:else}
    <div class="toolbar" id="skills-toolbar">
      <p class="summary">{summary}</p>
      <div class="seg" role="group" aria-label="Show">
        {#each FILTERS as f (f.id)}
          <button type="button" aria-pressed={filter === f.id} onclick={() => (filter = f.id)}>{f.label} <span class="count">{counts[f.id]}</span></button>
        {/each}
      </div>
      <label class="input-wrap search">
        <MagnifyingGlass size={16} aria-hidden="true" />
        <input type="search" bind:value={query} aria-label="Search skills" placeholder="Search by name or description" spellcheck="false" />
      </label>
    </div>

    {#if rows.length === 0}
      <p class="hint">{emptyText}</p>
    {:else}
      <div class="table-wrap">
        <table id="skill-matrix" aria-busy={scanning}>
          <caption class="sr-only">Skills in each folder</caption>
          <thead>
            <tr>
              <th scope="col" class="skill-col">Skill</th>
              {#each scan.roots as r (r.id)}
                <th scope="col" title={r.path}>
                  <span class="root-head">
                    {#if r.cli}<CliMark kind={r.cli} small />{:else}<span class="climark sm" aria-hidden="true"><FolderSimple size={14} /></span>{/if}
                    <b>{r.label}</b>
                  </span>
                  {#if !r.exists}<span class="chip idle">Folder not found</span>{/if}
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
                  {#if row.variants > 1}<span class="versions">{row.variants} versions</span>{/if}
                </th>
                {#each scan.roots as r (r.id)}
                  <td>
                    {#if r.exists}{@render cellView(cellFor(row, r.id))}{:else}<span class="sr-only">Folder not found</span>{/if}
                  </td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    <p class="note">AI Remote never copies skills for you. Open a skill to get the command, run it in your own terminal, then press Rescan.</p>
  {/if}
</main>

<Dialog bind:open={detailOpen} labelledby="skill-title" wide>
  {#if scan && detail}
    <div class="d-body">
      <div class="row">
        <h2 id="skill-title" class="grow">{detail.name}</h2>
        <button class="icon-btn" type="button" aria-label="Close" onclick={() => (detailOpen = false)}><X size={18} aria-hidden="true" /></button>
      </div>
      {#if detail.description}<p class="d-desc">{detail.description}</p>{/if}

      <ul class="where" aria-label="Folders">
        {#each scan.roots as r (r.id)}
          {@const e = entryIn(detail, r.id)}
          <li>
            {#if r.cli}<CliMark kind={r.cli} small />{:else}<span class="climark sm" aria-hidden="true"><FolderSimple size={14} /></span>{/if}
            <span class="grow">
              <b>{r.label}</b>
              <span class="mono path" title={e?.path ?? r.path}>{shortPath(e?.path ?? joinPath(scan.shell, r.path, detail.name))}</span>
              {#if e?.linkTarget}<span class="mono path" title={e.linkTarget}>links to {shortPath(e.linkTarget)}</span>{/if}
            </span>
            {#if r.exists}{@render cellView(cellFor(detail, r.id))}{:else}<span class="chip idle">Folder not found</span>{/if}
          </li>
        {/each}
      </ul>

      {#if sources.length === 0}
        <p class="meta">No folder has a readable copy of this skill, so there is nothing to copy from yet.</p>
      {:else if steps.length === 0}
        <p class="all-same"><CheckCircle size={16} aria-hidden="true" />Every folder that can take this skill already holds this version.</p>
      {:else}
        <div class="field">
          <label class="label" for="skill-source">Copy from</label>
          <select class="select" id="skill-source" bind:value={sourceRoot}>
            {#each sources as s (s.rootId)}
              <option value={s.rootId}>{rootLabel(s.rootId)}{detail.variants > 1 ? `, Version ${s.variant}` : ""}</option>
            {/each}
          </select>
        </div>
        <div class="steps">
          {#each steps as s (s.rootId)}
            {@const label = rootLabel(s.rootId)}
            <section class="step" aria-labelledby="step-{s.rootId}">
              <h3 id="step-{s.rootId}">{STEP_TITLE[s.kind](label)}</h3>
              {#if s.kind === "nested"}
                <p class="meta">This folder has a link inside it, and removing it with a command could delete the files that link points to. Replace it by hand, then press Rescan.</p>
              {:else if s.kind === "link"}
                <p class="meta">
                  {#if s.target}This folder links to <span class="mono">{shortPath(s.target)}</span>. Update it there.{:else}This folder is a link whose target is gone. Remove the link, then press Rescan to get a copy command.{/if}
                </p>
              {:else}
                {#if s.kind === "replace"}<p class="meta">Replaces the folder in {label}. Files that exist only in that copy are deleted.</p>{/if}
                <div class="cmd"><small>{scan.shell === "powershell" ? "PowerShell" : "Terminal"}</small><code>{s.command}</code></div>
                <div><button class="btn secondary sm" type="button" onclick={() => copy(s.command, s.rootId)}><Copy size={16} aria-hidden="true" />Copy command</button></div>
              {/if}
            </section>
          {/each}
        </div>
      {/if}

      <div class="d-foot">
        <span class="meta grow">Esc to close</span>
        <button class="btn secondary" type="button" onclick={() => (detailOpen = false)}>Close</button>
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
