<script lang="ts">
  import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
  import CheckCircle from "phosphor-svelte/lib/CheckCircle";
  import GitBranch from "phosphor-svelte/lib/GitBranch";
  import { onMount, untrack } from "svelte";
  import { api, errorText, type GitBranches } from "./api";
  import Dialog from "./Dialog.svelte";
  import { refocus } from "./focus";
  import { ago, folderName } from "./format";
  import { t, tb } from "./i18n.svelte";
  import { showToast } from "./store.svelte";
  import type { GitWatch } from "./workspace.svelte";

  let {
    id,
    cwd,
    watch,
    busyCli,
  }: {
    id: string;
    cwd: string;
    watch: GitWatch;
    /** The CLI of this session while it is live: a switch would change the files under it. */
    busyCli: string | null;
  } = $props();

  const folder = $derived(folderName(cwd));
  const st = $derived(watch.status);

  let data = $state<GitBranches | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");
  let panel: HTMLDivElement | undefined = $state();

  async function load() {
    try {
      data = await api.gitBranches(id);
      loadState = "ready";
    } catch (e) {
      loadError = errorText(e);
      loadState = "error";
    }
  }

  onMount(load);

  // A commit or a switch made elsewhere moves HEAD; the lists follow it. The first status to
  // arrive only tells where HEAD was when the lists were read.
  let seenHead: string | null = null;
  $effect(() => {
    if (!st) return;
    const head = `${st.branch}:${st.head}`;
    const first = seenHead === null;
    if (head === seenHead) return;
    seenHead = head;
    if (!first) untrack(load);
  });

  const dirty = $derived(Boolean(st?.changes.some((c) => c.code !== "?")));
  const blocked = $derived(busyCli ? t("workspace.branch.blockedLive", { cli: busyCli }) : dirty ? t("workspace.branch.blockedDirty") : "");

  const upstreamLine = $derived.by(() => {
    if (!st?.upstream) return t("workspace.branch.noUpstream");
    const parts = [st.ahead ? t("workspace.branch.ahead", { n: st.ahead }) : "", st.behind ? t("workspace.branch.behind", { n: st.behind }) : ""].filter(Boolean);
    return `${t("workspace.branch.tracks", { upstream: st.upstream })} · ${parts.length ? parts.join(", ") : t("workspace.branch.upToDate")}`;
  });
  const gone = $derived(Boolean(data?.branches.find((b) => b.current)?.gone));

  let pick = $state<string | null>(null);
  let confirmOpen = $state(false);
  let switching = $state(false);

  function ask(branch: string) {
    pick = branch;
    confirmOpen = true;
  }

  async function confirm() {
    const branch = pick;
    if (!branch) return;
    confirmOpen = false;
    switching = true;
    try {
      await api.gitSwitch(id, branch);
      showToast(t("workspace.branch.switched", { folder, branch }));
    } catch (e) {
      showToast(tb(errorText(e)));
    } finally {
      switching = false;
      await Promise.all([watch.refresh(), load()]);
      // The Switch button that opened the dialog is now the Current mark.
      refocus(panel);
    }
  }

  function refresh() {
    watch.refresh();
    load();
  }
</script>

<div class="branch-panel" bind:this={panel}>
  <div class="side-head">
    <h3>{t("workspace.tab.branch")}</h3>
    <button class="icon-btn sm" type="button" aria-label={t("workspace.refreshNamed", { what: t("workspace.branch.branches") })} title={t("workspace.refresh")} onclick={refresh}>
      <ArrowClockwise size={16} aria-hidden="true" />
    </button>
  </div>

  {#if !st && watch.state === "error"}
    <div class="side-state" role="alert">
      <b>{t("workspace.gitFailed")}</b>
      <p>{tb(watch.error)}</p>
      <button class="btn secondary sm" type="button" onclick={refresh}>{t("sessions.tryAgain")}</button>
    </div>
  {:else if !st}
    <p class="side-note" role="status">{t("workspace.gitLoading")}</p>
  {:else if !st.repo}
    <div class="side-state">
      <b>{t("workspace.notRepo", { folder })}</b>
      <p>{t("workspace.notRepoBody")}</p>
    </div>
  {:else}
    <div class="now" id="branch-current">
      <GitBranch size={18} aria-hidden="true" />
      <div class="grow">
        <b class="mono">{st.branch ?? t("workspace.branch.detached", { head: st.head ?? "" })}</b>
        <span>{gone ? t("workspace.branch.gone") : upstreamLine}</span>
      </div>
    </div>

    {#if blocked}<p class="side-note" id="branch-blocked">{blocked}</p>{/if}

    <div class="side-group">
      <h4>{t("workspace.branch.branches")}</h4>
      {#if !data && loadState === "error"}
        <p class="err-text" role="alert">{t("workspace.branch.failed")}: {tb(loadError)}</p>
        <button class="btn secondary sm" type="button" onclick={load}>{t("sessions.tryAgain")}</button>
      {:else if !data}
        <p class="side-note" role="status">{t("workspace.branch.loading")}</p>
      {:else if data.branches.length === 0}
        <p class="side-note">{t("workspace.branch.none")}</p>
      {:else}
        <ul class="list" id="branch-list">
          {#each data.branches as b (b.name)}
            <li class="branch">
              <div class="grow">
                <b class="mono" title={b.name}>{b.name}</b>
                {#if b.subject}<span class="subject" title={b.subject}>{b.subject}</span>{/if}
                <span class="when">{ago(b.at, Date.now())}</span>
              </div>
              {#if b.current}
                <span class="current"><CheckCircle size={16} weight="fill" aria-hidden="true" />{t("workspace.branch.current")}</span>
              {:else}
                <button
                  class="btn secondary sm"
                  type="button"
                  disabled={Boolean(blocked) || switching}
                  aria-label={t("workspace.branch.switchNamed", { branch: b.name })}
                  aria-describedby={blocked ? "branch-blocked" : undefined}
                  onclick={() => ask(b.name)}
                >
                  {t("workspace.branch.switch")}
                </button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="side-group">
      <h4>{t("workspace.branch.commits")}</h4>
      {#if data && data.commits.length === 0}
        <p class="side-note">{t("workspace.branch.noCommits")}</p>
      {:else if data}
        <ul class="list" id="commit-list">
          {#each data.commits as c (c.hash)}
            <li class="commit">
              <span class="hash mono">{c.hash}</span>
              <div class="grow">
                <span class="subject-full">{c.subject}</span>
                <span class="when">{t("workspace.branch.commitMeta", { author: c.author, when: ago(c.at, Date.now()) })}</span>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<Dialog bind:open={confirmOpen} labelledby="switch-title">
  <div class="d-body">
    <h2 id="switch-title">{t("workspace.branch.confirmTitle", { branch: pick ?? "" })}</h2>
    <p class="meta" style="margin:0;font-size:14px">{t("workspace.branch.confirmBody", { folder, branch: pick ?? "" })}</p>
    <div class="d-foot">
      <span class="grow"></span>
      <button class="btn secondary" type="button" onclick={() => (confirmOpen = false)}>
        {st?.branch ? t("workspace.branch.stay", { branch: st.branch }) : t("workspace.branch.cancel")}
      </button>
      <button class="btn primary" type="button" id="btn-switch-branch" onclick={confirm}>
        <GitBranch size={16} aria-hidden="true" />{t("workspace.branch.switchNamed", { branch: pick ?? "" })}
      </button>
    </div>
  </div>
</Dialog>

<style>
  .branch-panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .now {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .now :global(svg) {
    margin-top: 1px;
    color: var(--forest);
  }
  .now .grow {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
  }
  .now b {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  h4 {
    margin: 0;
    font-size: 13px;
    font-weight: 800;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .branch,
  .commit {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12px;
  }
  .branch .grow,
  .commit .grow {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .branch b {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .subject-full {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .when {
    color: var(--ink-2);
  }
  .current {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-weight: 700;
    min-height: 32px;
  }
  .current :global(svg) {
    color: var(--forest);
  }
  .hash {
    flex: none;
    padding-top: 1px;
  }
</style>
