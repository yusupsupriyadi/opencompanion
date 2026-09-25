import type { SkillScan } from "$lib/api";

const H = String.raw`C:\Users\me`;

/** Five folders, one missing; one skill shared as-is, one with two versions and a link, one broken. */
export function skillScan(over: Partial<SkillScan> = {}): SkillScan {
  return {
    shell: "powershell",
    roots: [
      { id: "claude", label: "Claude Code", cli: "claude", path: `${H}\\.claude\\skills`, exists: true },
      { id: "codex", label: "Codex CLI", cli: "codex", path: `${H}\\.codex\\skills`, exists: true },
      { id: "agents", label: "Shared", cli: null, path: `${H}\\.agents\\skills`, exists: true },
      { id: "opencode", label: "OpenCode", cli: "opencode", path: `${H}\\.config\\opencode\\skills`, exists: true },
      { id: "gemini", label: "Gemini CLI", cli: "gemini", path: `${H}\\.gemini\\skills`, exists: false },
    ],
    skills: [
      {
        name: "antislop",
        description: "Anti Slop filter for UI work",
        variants: 1,
        entries: ["claude", "codex", "opencode"].map((rootId) => ({
          rootId,
          path: `${H}\\${rootId === "opencode" ? ".config\\opencode" : "." + rootId}\\skills\\antislop`,
          hash: "h1",
          variant: "A",
          modifiedAt: Date.now() - 3 * 3_600_000,
          linkTarget: null,
          containsLinks: false,
          problem: null,
        })),
      },
      {
        name: "browseros-neo",
        description: "Drive the user's own browser",
        variants: 2,
        entries: [
          { rootId: "claude", path: `${H}\\.claude\\skills\\browseros-neo`, hash: "h2", variant: "A", modifiedAt: Date.now() - 60_000, linkTarget: null, containsLinks: false, problem: null },
          { rootId: "agents", path: `${H}\\.agents\\skills\\browseros-neo`, hash: "h3", variant: "B", modifiedAt: Date.now() - 2 * 3_600_000, linkTarget: null, containsLinks: false, problem: null },
          {
            rootId: "opencode",
            path: `${H}\\.config\\opencode\\skills\\browseros-neo`,
            hash: "h3",
            variant: "B",
            modifiedAt: Date.now() - 2 * 3_600_000,
            linkTarget: `${H}\\.agents\\skills\\browseros-neo`,
            containsLinks: false,
            problem: null,
          },
        ],
      },
      {
        name: "canvas-design",
        description: null,
        variants: 0,
        entries: [{ rootId: "agents", path: `${H}\\.agents\\skills\\canvas-design`, hash: null, variant: null, modifiedAt: null, linkTarget: null, containsLinks: false, problem: "noSkillMd" }],
      },
    ],
    ...over,
  };
}
