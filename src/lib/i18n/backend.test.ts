import { expect, test } from "vitest";
import { translateBackend } from "./backend";

test("backend messages read in Indonesian, with their names and numbers kept", () => {
  expect(translateBackend("This folder does not exist.", "id")).toBe("Folder ini tidak ada.");
  expect(translateBackend("Exited with code 1", "id")).toBe("Keluar dengan kode 1");
  expect(translateBackend("Codex CLI is not installed on this computer.", "id")).toBe("Codex CLI tidak terpasang di komputer ini.");
  expect(translateBackend("Approved Write on Pixel 8", "id")).toBe("Disetujui: Write dari Pixel 8");
  expect(translateBackend("The provider did not answer within 300 s.", "id")).toBe("Provider tidak menjawab dalam 300 detik.");
  expect(translateBackend("Claude Code did not answer within 300 s.", "id")).toBe("Claude Code tidak menjawab dalam 300 detik.");
  // The CLI's own words after the colon stay as the CLI wrote them.
  expect(translateBackend("Codex CLI could not answer: unexpected status 401 Unauthorized", "id")).toBe(
    "Codex CLI tidak bisa menjawab: unexpected status 401 Unauthorized",
  );
});

test("English and text the backend did not write pass through", () => {
  expect(translateBackend("This folder does not exist.", "en")).toBe("This folder does not exist.");
  expect(translateBackend("Running bun run test", "id")).toBe("Running bun run test");
  expect(translateBackend("", "id")).toBe("");
});
