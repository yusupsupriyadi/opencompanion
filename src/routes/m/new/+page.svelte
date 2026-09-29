<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import CaretLeft from "phosphor-svelte/lib/CaretLeft";
  import Play from "phosphor-svelte/lib/Play";
  import SpinnerGap from "phosphor-svelte/lib/SpinnerGap";
  import { onMount, tick } from "svelte";
  import type { CliKind, Mode, PermMode, SessionInfo } from "$lib/api";
  import { CLI_LABEL, folderName } from "$lib/format";
  import { t, tb } from "$lib/i18n.svelte";
  import { call, type PhoneOptions } from "$lib/phone.svelte";
  import PhoneSessionFields, { OTHER_FOLDER as OTHER } from "$lib/PhoneSessionFields.svelte";

  // `?cwd=` starts in that folder.

  let options = $state<PhoneOptions | null>(null);
  let loadState = $state<"loading" | "ready" | "error">("loading");
  let loadError = $state("");

  let cli = $state<CliKind>("claude");
  let folder = $state("");
  let typed = $state("");
  // Headless by default: its steps read well on a phone and it takes follow-ups.
  let mode = $state<Mode>("headless");
  let prompt = $state("");
  let permissionMode = $state<PermMode>("ask");
  let folderError = $state("");
  let promptError = $state("");
  let failure = $state("");
  let busy = $state(false);

  const installed = $derived(options?.clis.filter((c) => c.path) ?? []);
  const cwd = $derived((folder === OTHER ? typed : folder).trim());
  const headless = $derived(mode === "headless");
  const label = $derived.by(() => {
    const where = cwd ? folderName(cwd) : "…";
    return t("phone.new.start", { cli: CLI_LABEL[cli], folder: where });
  });

  async function load() {
    loadState = "loading";
    try {
      const o = await call<PhoneOptions>("/api/options");
      options = o;
      const usable = o.clis.filter((c) => c.path);
      cli = usable.find((c) => c.kind !== "gemini")?.kind ?? usable[0]?.kind ?? "claude";
      const start = page.url.searchParams.get("cwd") || "";
      if (start && !o.folders.some((f) => f.path === start)) {
        folder = OTHER;
        typed = start;
      } else {
        folder = start || o.folders[0]?.path || OTHER;
      }
      permissionMode = o.permissionMode;
      loadState = "ready";
    } catch (e) {
      loadState = "error";
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);

  async function focus(id: string) {
    await tick();
    document.getElementById(id)?.focus();
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    failure = "";
    if (!cwd) {
      folderError = t("phone.new.folderRequired");
      focus(folder === OTHER ? "pn-path" : "pn-folder");
      return;
    }
    if (headless && !prompt.trim()) {
      promptError = t("phone.new.promptRequired");
      focus("pn-prompt");
      return;
    }
    if (headless && cli === "gemini") {
      failure = t("phone.new.geminiHeadless");
      return;
    }
    busy = true;
    try {
      const body = JSON.stringify({ cli, cwd, mode, prompt: prompt.trim(), permissionMode });
      const r = await call<{ session: SessionInfo }>("/api/sessions", { method: "POST", body });
      await goto(`/m/session?id=${r.session.id}`, { replaceState: true });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg === "This folder does not exist.") {
        folderError = msg;
        focus(folder === OTHER ? "pn-path" : "pn-folder");
      } else {
        failure = msg;
      }
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>{t("phone.newSession")} · OpenCompanion</title></svelte:head>

<header class="bar m-bar-tight">
  <a class="m-icon-btn m-back" href="/m" aria-label={t("phone.session.back")} title={t("phone.session.back")}>
    <CaretLeft size={20} aria-hidden="true" />
  </a>
  <h1 class="m-bar-title">{t("phone.newSession")}</h1>
</header>
<main class="content dense" id="phone-new-session">
  {#if loadState === "loading"}
    <p class="m-p m-inline" role="status"><SpinnerGap size={16} class="spin" aria-hidden="true" />{t("phone.new.loading")}</p>
  {:else if loadState === "error"}
    <p class="err-text" role="alert" style="margin:0">{tb(loadError)}</p>
    <button class="btn secondary block" type="button" onclick={load}>{t("phone.tryAgain")}</button>
  {:else if options}
    <p class="m-lead">
      {t("phone.new.lead")}
    </p>
    <form class="m-form" novalidate onsubmit={submit}>
      <PhoneSessionFields
        {options}
        bind:cli
        bind:folder
        bind:typed
        bind:mode
        bind:prompt
        bind:permissionMode
        bind:folderError
        bind:promptError
        idPrefix="pn"
      />

      {#if failure}<p class="err-text" role="alert" style="margin:0">{tb(failure)}</p>{/if}

      <button class="btn primary block" type="submit" id="btn-start-session" disabled={busy || installed.length === 0}>
        {#if busy}<SpinnerGap size={16} class="spin" aria-hidden="true" />{:else}<Play size={16} aria-hidden="true" />{/if}<span style="overflow:hidden;text-overflow:ellipsis">{busy ? t("phone.new.starting") : label}</span>
      </button>
    </form>
  {/if}
</main>

<style>
  .m-back {
    margin-left: -8px;
  }
  .m-bar-title {
    flex: 1;
    min-width: 0;
    font-size: 17px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .m-lead {
    margin: 0;
    font-size: 13px;
    color: var(--ink-2);
    line-height: 1.45;
  }
</style>
