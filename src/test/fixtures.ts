import { invoke } from "@tauri-apps/api/core";
import { vi } from "vitest";
import type { CliInstall, SessionView } from "$lib/api";

export const CLIS: CliInstall[] = [
  { kind: "claude", label: "Claude Code", path: "C:\\Users\\me\\.local\\bin\\claude.exe", version: "2.1.282", tested: true, error: null },
  { kind: "codex", label: "Codex CLI", path: "C:\\Codex\\codex.exe", version: "0.153.4", tested: true, error: null },
  { kind: "opencode", label: "OpenCode", path: null, version: null, tested: false, error: null },
  { kind: "gemini", label: "Gemini CLI", path: null, version: null, tested: false, error: null },
  { kind: "ccs", label: "CCS", path: null, version: null, tested: false, error: null },
  { kind: "pi", label: "Pi", path: null, version: null, tested: false, error: null },
  { kind: "omp", label: "omp", path: null, version: null, tested: false, error: null },
  { kind: "cursor", label: "Cursor CLI", path: null, version: null, tested: false, error: null },
];

export function session(over: Partial<SessionView> = {}): SessionView {
  return {
    id: "s1",
    cli: "claude",
    cwd: "C:\\Users\\me\\Project\\uninote",
    mode: "headless",
    title: "Add a dark mode toggle",
    prompt: "Add a dark mode toggle",
    status: "running",
    pid: 42,
    cliSessionId: null,
    startedAt: Date.now() - 5 * 60_000,
    endedAt: null,
    exitCode: null,
    lastEvent: "Write a.txt",
    waiting: null,
    source: "manual",
    permissionMode: null,
    updatedAt: Date.now(),
    marks: [],
    ...over,
  };
}

type Handler = (args: Record<string, unknown> | undefined) => unknown;

/** Routes `invoke(cmd, args)` to per-command handlers and records every call. */
export function backend(handlers: Record<string, Handler>) {
  const mock = vi.mocked(invoke);
  mock.mockReset();
  mock.mockImplementation(async (cmd: string, args?: unknown) => {
    const h = handlers[cmd];
    if (!h) return undefined;
    const out = h(args as Record<string, unknown> | undefined);
    if (out instanceof Error) throw out.message;
    return out;
  });
  return {
    calls: (cmd: string) => mock.mock.calls.filter(([c]) => c === cmd).map(([, a]) => a as Record<string, unknown>),
  };
}
