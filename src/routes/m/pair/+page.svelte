<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import { onMount, tick } from "svelte";
  import { call, connect, loadSessions, phone, setToken } from "$lib/phone.svelte";

  let digits = $state(["", "", "", "", "", ""]);
  let name = $state("");
  let failure = $state("");
  let busy = $state(false);
  const fromQr = page.url.searchParams.get("code")?.replace(/\D/g, "").slice(0, 6) ?? "";

  function guessName() {
    const ua = navigator.userAgent;
    const device = /iPhone/.test(ua) ? "iPhone" : /iPad/.test(ua) ? "iPad" : /Android/.test(ua) ? "Android phone" : "Phone";
    const browser = /CriOS|Chrome/.test(ua) ? "Chrome" : /FxiOS|Firefox/.test(ua) ? "Firefox" : /Safari/.test(ua) ? "Safari" : "browser";
    return `${device} · ${browser}`;
  }

  onMount(() => {
    name = guessName();
    if (fromQr.length === 6) digits = fromQr.split("");
    phone.unpaired = false;
  });

  async function focusDigit(i: number) {
    await tick();
    document.getElementById(`c${i}`)?.focus();
  }

  function oninput(i: number, e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    const clean = el.value.replace(/\D/g, "");
    if (clean.length > 1) {
      // A pasted code fills the boxes from here.
      clean.slice(0, 6 - i).split("").forEach((d, k) => (digits[i + k] = d));
      focusDigit(Math.min(5, i + clean.length));
      return;
    }
    digits[i] = clean;
    failure = "";
    if (clean && i < 5) focusDigit(i + 1);
  }

  function onkeydown(i: number, e: KeyboardEvent) {
    if (e.key === "Backspace" && !digits[i] && i > 0) focusDigit(i - 1);
  }

  async function pair(e: SubmitEvent) {
    e.preventDefault();
    const code = digits.join("");
    if (code.length !== 6) {
      failure = "Enter all 6 digits shown on your computer.";
      focusDigit(digits.findIndex((d) => !d));
      return;
    }
    busy = true;
    failure = "";
    try {
      const r = await call<{ token: string }>("/api/pair", { method: "POST", body: JSON.stringify({ code, name }) });
      setToken(r.token);
      phone.unpaired = false;
      await loadSessions();
      connect();
      await goto("/m");
    } catch (err) {
      failure = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>Pair this phone · AI Remote</title></svelte:head>

<header class="bar"><span class="brand grow">AI Remote <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-pair">
  <img class="art" src="/meadow-day.webp" alt="" aria-hidden="true" />
  <h1 class="m-h1">Pair this phone</h1>
  {#if fromQr.length === 6}
    <p class="m-p">The code from your computer is filled in. Check the name, then press Pair.</p>
  {:else}
    <p class="m-p">On your computer, open AI Remote, go to Settings, turn on Phone access and scan the code there with your camera app. Or type the 6-digit code below.</p>
  {/if}

  <form onsubmit={pair} novalidate style="display:flex;flex-direction:column;gap:14px">
    <fieldset style="border:0;padding:0;margin:0;display:flex;flex-direction:column;gap:10px">
      <legend class="label" style="font-weight:800;padding:0;margin-bottom:10px">Pairing code</legend>
      <div class="codes">
        {#each digits as d, i (i)}
          <input
            id="c{i}"
            value={d}
            inputmode="numeric"
            maxlength="6"
            aria-label="Digit {i + 1}"
            autocomplete="one-time-code"
            aria-invalid={failure ? "true" : undefined}
            oninput={(e) => oninput(i, e)}
            onkeydown={(e) => onkeydown(i, e)}
          />
        {/each}
      </div>
    </fieldset>
    <div class="field">
      <label class="label" for="device-name">Name for this phone</label>
      <input class="input" id="device-name" bind:value={name} maxlength="60" autocomplete="off" />
      <p class="help">Shown in AI Remote Settings, where you can remove this phone later.</p>
    </div>
    {#if failure}<p class="error" role="alert" style="margin:0">{failure}</p>{/if}
    <button class="btn primary block" type="submit" disabled={busy}>{busy ? "Pairing…" : "Pair"}</button>
  </form>
  <p class="small" style="margin:0">This phone talks only to your computer, over your own network.</p>
</main>
