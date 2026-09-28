import { expect, test } from "vitest";
import { translateBackend } from "./backend";

test("backend messages read in Indonesian, with their names and numbers kept", () => {
  expect(translateBackend("This folder does not exist.", "id")).toBe("Folder ini tidak ada.");
  expect(translateBackend("Exited with code 1", "id")).toBe("Keluar dengan kode 1");
  expect(translateBackend("Codex CLI is not installed on this computer.", "id")).toBe("Codex CLI tidak terpasang di komputer ini.");
  expect(translateBackend("That shell is not installed on this computer.", "id")).toBe("Shell itu tidak terpasang di komputer ini.");
  expect(translateBackend("Approved Write on Pixel 8", "id")).toBe("Disetujui: Write dari Pixel 8");
  expect(translateBackend("The provider did not answer within 300 s.", "id")).toBe("Provider tidak menjawab dalam 300 detik.");
  expect(translateBackend("Claude Code did not answer within 300 s.", "id")).toBe("Claude Code tidak menjawab dalam 300 detik.");
  // The CLI's own words after the colon stay as the CLI wrote them.
  expect(translateBackend("Codex CLI could not answer: unexpected status 401 Unauthorized", "id")).toBe(
    "Codex CLI tidak bisa menjawab: unexpected status 401 Unauthorized",
  );
});

test("login item errors on macOS and Linux read in Indonesian", () => {
  expect(translateBackend("Could not add OpenCompanion to the login items: Permission denied (os error 13)", "id")).toBe(
    "OpenCompanion tidak bisa ditambahkan ke daftar mulai saat masuk: Permission denied (os error 13)",
  );
  expect(translateBackend("Could not remove OpenCompanion from the login items: Read-only file system (os error 30)", "id")).toBe(
    "OpenCompanion tidak bisa dihapus dari daftar mulai saat masuk: Read-only file system (os error 30)",
  );
  expect(translateBackend("Your home folder could not be found, so the login item could not be written.", "id")).toBe(
    "Folder home Anda tidak ditemukan, jadi OpenCompanion tidak bisa ditambahkan ke daftar mulai saat masuk.",
  );
  expect(translateBackend("Starting at sign-in is not available on this system.", "id")).toBe("Mulai saat masuk tidak tersedia di sistem ini.");
});

test("file and git errors read in Indonesian, with git's own words kept", () => {
  expect(translateBackend("Git is not installed or not on PATH.", "id")).toBe("Git tidak terpasang atau tidak ada di PATH.");
  expect(translateBackend("That path is outside this session's folder.", "id")).toBe("Path itu berada di luar folder sesi ini.");
  expect(translateBackend("Git could not switch branches: error: pathspec 'x' did not match", "id")).toBe(
    "Git tidak bisa pindah branch: error: pathspec 'x' did not match",
  );
  expect(translateBackend("Commit or stash the changes in this folder before switching branches.", "id")).toBe(
    "Commit atau stash perubahan di folder ini sebelum pindah branch.",
  );
});

test("English and text the backend did not write pass through", () => {
  expect(translateBackend("This folder does not exist.", "en")).toBe("This folder does not exist.");
  expect(translateBackend("Running bun run test", "id")).toBe("Running bun run test");
  expect(translateBackend("", "id")).toBe("");
});
