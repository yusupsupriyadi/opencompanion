// The phone's on-screen keyboard, read from the visual viewport. A composer pinned to the bottom
// rides above it (DESIGN.md section 12: a focused field is never covered), and Enter only sends
// from a hardware keyboard.

/** `inset` is how far the on-screen keyboard reaches up from the bottom of the page, in px. */
export const keyboard = $state({ inset: 0 });

// The tallest view seen at the current width is the view with no on-screen keyboard.
let tallest = 0;
let width = 0;

function measure() {
  const vv = window.visualViewport;
  if (!vv) return;
  // A new width is a turned phone, whose tallest view starts over.
  if (vv.width !== width) {
    width = vv.width;
    tallest = vv.height;
  } else {
    tallest = Math.max(tallest, vv.height);
  }
  keyboard.inset = Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop));
}

/** Follows the visual viewport while a screen with a bottom composer is open. Returns the stop. */
export function watchKeyboard(): () => void {
  const vv = window.visualViewport;
  if (!vv) return () => undefined;
  measure();
  vv.addEventListener("resize", measure);
  vv.addEventListener("scroll", measure);
  return () => {
    vv.removeEventListener("resize", measure);
    vv.removeEventListener("scroll", measure);
    keyboard.inset = 0;
  };
}

/** Space an on-screen keyboard takes at least; the iPad's shortcut bar over a hardware keyboard is less. */
const ON_SCREEN_KEYBOARD = 120;

/**
 * Keys come from a hardware keyboard: nothing on screen took room while typing, or, without a
 * visual viewport to ask, the device has a fine pointer. Unsure means no, so Enter adds a line
 * and the Send button sends.
 */
export function hardwareKeyboard(): boolean {
  const vv = window.visualViewport;
  if (vv && tallest > 0) return tallest - vv.height < ON_SCREEN_KEYBOARD;
  return window.matchMedia?.("(pointer: fine)").matches ?? false;
}
