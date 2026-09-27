<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import Cloud from "phosphor-svelte/lib/Cloud";
  import { onMount, tick } from "svelte";
  import { t, tb } from "$lib/i18n.svelte";
  import { call, connect, iosHomeScreen, loadSessions, phone, setToken } from "$lib/phone.svelte";

  let digits = $state(["", "", "", "", "", ""]);
  let name = $state("");
  let nameTyped = false;
  let failure = $state("");
  let busy = $state(false);
  const fromQr = page.url.searchParams.get("code")?.replace(/\D/g, "").slice(0, 6) ?? "";
  // Pairing again after Add to Home Screen: a code from the QR would open Safari, so the code is typed here.
  const homeScreen = iosHomeScreen();

  function guessName() {
    const ua = navigator.userAgent;
    const device = /iPhone/.test(ua) ? "iPhone" : /iPad/.test(ua) ? "iPad" : /Android/.test(ua) ? t("phone.pair.androidPhone") : t("phone.pair.phone");
    // Safari and the Home Screen app are two devices in Settings on the desktop; the name tells them apart.
    const browser = homeScreen
      ? t("phone.pair.homeScreen")
      : /CriOS|Chrome/.test(ua) ? "Chrome" : /FxiOS|Firefox/.test(ua) ? "Firefox" : /Safari/.test(ua) ? "Safari" : t("phone.pair.browser");
    return `${device} · ${browser}`;
  }

  // The desktop's language arrives after this page opens, so the suggested name follows it until the user types their own.
  $effect(() => {
    const suggested = guessName();
    if (!nameTyped) name = suggested;
  });

  onMount(() => {
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
      failure = t("phone.pair.incomplete");
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

<svelte:head><title>{t("phone.pair.title")} · OpenCompanion</title></svelte:head>

<header class="bar"><span class="brand grow">OpenCompanion <Cloud size={20} aria-hidden="true" /></span></header>
<main class="content" id="phone-pair">
  <img class="art" src="/meadow-day.png" alt="" aria-hidden="true" />
  <h1 class="m-h1">{t("phone.pair.title")}</h1>
  {#if fromQr.length === 6}
    <p class="m-p">{t("phone.pair.fromQr")}</p>
  {:else if homeScreen}
    <p class="m-p">{t("phone.pair.homeScreenHowTo")}</p>
  {:else}
    <p class="m-p">{t("phone.pair.howTo")}</p>
  {/if}

  <form onsubmit={pair} novalidate style="display:flex;flex-direction:column;gap:14px">
    <fieldset style="border:0;padding:0;margin:0;display:flex;flex-direction:column;gap:10px">
      <legend class="label" style="font-weight:800;padding:0;margin-bottom:10px">{t("phone.pair.code")}</legend>
      <div class="codes">
        {#each digits as d, i (i)}
          <input
            id="c{i}"
            value={d}
            inputmode="numeric"
            maxlength="6"
            aria-label={t("phone.pair.digit", { n: i + 1 })}
            autocomplete="one-time-code"
            aria-invalid={failure ? "true" : undefined}
            oninput={(e) => oninput(i, e)}
            onkeydown={(e) => onkeydown(i, e)}
          />
        {/each}
      </div>
    </fieldset>
    <div class="field">
      <label class="label" for="device-name">{t("phone.pair.name")}</label>
      <input class="input" id="device-name" bind:value={name} oninput={() => (nameTyped = true)} maxlength="60" autocomplete="off" />
      <p class="help">{t("phone.pair.nameHelp")}</p>
    </div>
    {#if failure}<p class="error" role="alert" style="margin:0">{tb(failure)}</p>{/if}
    <button class="btn primary block" type="submit" disabled={busy}>{busy ? t("phone.pair.pairing") : t("phone.pair.pair")}</button>
  </form>
  <p class="small" style="margin:0">{t("phone.pair.privacy")}</p>
</main>
