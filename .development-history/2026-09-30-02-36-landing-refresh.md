# Landing page refresh

## Summary

The landing page caught up with the app. The empty afternoon left by the Board now shows the session screen, and a new night scene shows Chat creating an automation. The facts sections list the three newer CLIs, the tested platforms and the new features.

## Changes

- `landing/index.html`: 15:00 scene (split bash beside Codex CLI running the tests, Changes panel, diff tab with syntax colors), 20:00 scene in the Dusk theme (automation card, Create, the new row with its next run). Timeline 0 to 116 (`END`), story 1220vh, sunset and dusk moved 5 later, clock and HUD scaled to `END`. Chat cards get titles and a Ready chip; the phone gets the icon New session button and the Automations tab. CCS, Pi and omp in the CLI list; features, pairing, privacy, limits, requirements and the footer (0.1.1) updated. Nine Phosphor icons copied from `phosphor-svelte`.

## Decisions

- The owner chose two new scenes (session screen in the afternoon, Automations at night) over text-only updates.
- At phone width the session mockup hides its side panel and stacks the two terminals, as the app does in a narrow window.

## Verification

- `node --check` on the page scripts: ok. HTML tag balance and duplicate ids (Python parser): none. Em dashes: none.
- `contrast-check.py` on 13 new pairs (diff rows, viewer bar, Dusk rows and chips, tab glass): all pass AA.

## Limitations

- Not opened in a browser: BrowserOS Neo was not connected, and a browser check needs the owner's approval. Scene positions, the scrub and the plain page are checked by reading the code only.

## Follow-up

- Browser check of the 15:00 and 20:00 scenes at desktop and phone width, and of the plain page (`?static=1`).
