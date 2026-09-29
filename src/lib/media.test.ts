import { expect, test } from "vitest";
import { mediaFor, zoomStep } from "./media";

test("pictures and PDFs are known by their extension, in any case", () => {
  expect(mediaFor("design/logo.PNG")).toMatchObject({ kind: "image", mime: "image/png", label: "PNG" });
  expect(mediaFor(String.raw`C:\docs\spec.pdf`)).toMatchObject({ kind: "pdf", mime: "application/pdf" });
  expect(mediaFor("static/favicon.svg")?.label).toBe("SVG");
  expect(mediaFor("photo.heic")).toBeNull();
  expect(mediaFor(".png")).toBeNull();
  expect(mediaFor("notes.md")).toBeNull();
});

test("zoom steps up and down from any level and stops at the ends", () => {
  expect(zoomStep(1, 1)).toBe(1.25);
  expect(zoomStep(1, -1)).toBe(0.75);
  // A fitted level between steps goes to the neighbouring step.
  expect(zoomStep(0.63, 1)).toBe(0.75);
  expect(zoomStep(0.63, -1)).toBe(0.5);
  expect(zoomStep(4, 1)).toBeNull();
  expect(zoomStep(0.25, -1)).toBeNull();
});
