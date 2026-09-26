import { tick } from "svelte";

/**
 * After an action whose button goes away (Approve, Run, Discard, a dialog whose opener was
 * deleted), focus would drop to the top of the document. This puts it on the first control
 * left in `within`, or else on the page's main region, so keyboard and screen reader users
 * carry on where they were. Focus that already moved somewhere is left alone.
 */
export async function refocus(within?: HTMLElement | null) {
  await tick();
  const at = document.activeElement;
  if (at && at !== document.body && at.isConnected) return;
  const control = within?.isConnected ? within.querySelector<HTMLElement>("a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled])") : null;
  if (control) {
    control.focus();
    return;
  }
  const main = document.querySelector<HTMLElement>("main");
  if (!main) return;
  if (!main.hasAttribute("tabindex")) main.setAttribute("tabindex", "-1");
  main.focus({ preventScroll: true });
}
