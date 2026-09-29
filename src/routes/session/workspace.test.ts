import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { MEDIA_TOO_LARGE, type FolderEntry, type GitChange, type GitStatus } from "$lib/api";
import { highlight, type Token } from "$lib/highlight";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../test/app-state.svelte";
import { CLIS, backend, session } from "../../test/fixtures";
import SessionPage from "./+page.svelte";

// Colors come from Shiki in the app; here a test hands in its own, and by default a file stays plain text.
vi.mock("$lib/highlight", async (actual) => ({ ...(await actual<typeof import("$lib/highlight")>()), highlight: vi.fn() }));

// pdf.js needs a real canvas; a test hands in a document whose pages report their size and draw nothing.
const pdf = vi.hoisted(() => ({ getDocument: vi.fn() }));
vi.mock("pdfjs-dist/legacy/build/pdf.mjs", () => ({
  GlobalWorkerOptions: { workerSrc: "" },
  getDocument: pdf.getDocument,
  TextLayer: class {
    render = async () => undefined;
    cancel() {}
  },
}));

const tok = (text: string, color = ""): Token => ({ text, color, italic: false, bold: false });

const dir = (name: string, path = name, ignored = false): FolderEntry => ({ name, path, dir: true, ignored });
const file = (name: string, path = name, ignored = false): FolderEntry => ({ name, path, dir: false, ignored });

function change(path: string, over: Partial<GitChange> = {}): GitChange {
  return { path, oldPath: null, code: "M", staged: false, unstaged: true, added: 3, removed: 1, ...over };
}

function gitStatus(over: Partial<GitStatus> = {}): GitStatus {
  return { repo: true, branch: "main", head: "abc1234", upstream: "origin/main", ahead: 0, behind: 0, changes: [], truncated: false, ...over };
}

const LISTING: Record<string, FolderEntry[]> = {
  "": [dir("src"), dir("build", "build", true), file("README.md")],
  src: [file("app.ts", "src/app.ts")],
};

function folder(extra: Record<string, (args: Record<string, unknown> | undefined) => unknown> = {}, status = gitStatus({ changes: [change("src/app.ts")] })) {
  const s = session({ status: "done", endedAt: Date.now() });
  app.sessions = [s];
  return backend({
    get_session: () => ({ session: s, events: [] }),
    terminal_list: () => [],
    folder_list: (a) => ({ entries: LISTING[String(a?.dir)] ?? [], truncated: false }),
    git_status: () => status,
    git_branches: () => ({
      repo: true,
      branches: [
        { name: "main", current: true, upstream: "origin/main", ahead: 0, behind: 0, gone: false, subject: "Fix the tray", at: Date.now() - 3_600_000 },
        { name: "feature", current: false, upstream: null, ahead: 0, behind: 0, gone: false, subject: "Try a thing", at: Date.now() - 7_200_000 },
      ],
      commits: [{ hash: "abc1234", subject: "Fix the tray", author: "Ana", at: Date.now() - 3_600_000 }],
    }),
    ...extra,
  });
}

const wideMedia = window.matchMedia;

/** Makes the window read as narrower than 720 for the MediaQuery the page builds on render. */
function narrowWindow() {
  window.matchMedia = ((media: string) => ({ ...wideMedia(media), matches: media.includes("max-width: 720px") })) as typeof window.matchMedia;
}

beforeEach(() => {
  vi.mocked(highlight).mockReset();
  pdf.getDocument.mockReset();
  URL.createObjectURL = vi.fn(() => "blob:picture");
  URL.revokeObjectURL = vi.fn();
  localStorage.removeItem("oc-session-side-tab");
  localStorage.removeItem("oc-session-side-hidden");
  localStorage.removeItem("oc-viewer-wrap");
  app.clis = CLIS;
  app.sessions = [];
  app.now = Date.now();
  setUrl("/session?id=s1");
});

afterEach(() => {
  window.matchMedia = wideMedia;
});

test("Files lists folders first, marks ignored and changed entries, and opens a file above the output", async () => {
  const calls = folder({ folder_read: () => ({ text: "const a = 1;\nconst b = 2;\n", size: 26, binary: false, tooLarge: false }) });
  render(SessionPage);

  expect(await screen.findByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");
  const tree = await screen.findByRole("tree", { name: "Files in uninote" });
  const build = within(tree).getByRole("treeitem", { name: /build/ });
  expect(build).toHaveTextContent("Ignored by .gitignore");
  const src = within(tree).getByRole("treeitem", { name: /^src/ });
  expect(src).toHaveAttribute("aria-expanded", "false");
  expect(src).toHaveTextContent("Holds uncommitted changes");

  await fireEvent.click(src);
  expect(calls.calls("folder_list")).toContainEqual({ id: "s1", dir: "src" });
  const app_ts = await within(tree).findByRole("treeitem", { name: /app\.ts/ });
  expect(app_ts).toHaveTextContent("Modified");
  expect(app_ts).toHaveAttribute("aria-level", "2");

  await fireEvent.click(app_ts);
  expect(await screen.findByText("const b = 2;")).toBeInTheDocument();
  expect(calls.calls("folder_read")).toEqual([{ id: "s1", path: "src/app.ts" }]);
  expect(screen.getByRole("region", { name: "src/app.ts, read-only" })).toBeInTheDocument();
  // The tab shows the file's type glyph before its name, hidden from screen readers.
  expect(document.querySelector("#view-tab-file .tab-icon svg")).toHaveAttribute("aria-hidden", "true");
  expect(document.getElementById("session-term")).not.toBeVisible();

  await fireEvent.click(screen.getByRole("button", { name: /^Claude Code\s*output$/ }));
  expect(document.getElementById("session-term")).toBeVisible();
  await fireEvent.click(screen.getByRole("button", { name: "Close app.ts" }));
  expect(screen.queryByRole("button", { name: /app\.ts/ })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Claude Code\s*output$/ })).toHaveAttribute("aria-current", "true");
});

test("the tree moves with the arrow keys", async () => {
  folder();
  render(SessionPage);
  const tree = await screen.findByRole("tree");
  const src = within(tree).getByRole("treeitem", { name: /^src/ });
  expect(src).toHaveAttribute("tabindex", "0");

  src.focus();
  await fireEvent.keyDown(src, { key: "ArrowRight" });
  expect(await within(tree).findByRole("treeitem", { name: /app\.ts/ })).toBeInTheDocument();
  await fireEvent.keyDown(src, { key: "ArrowDown" });
  expect(document.activeElement).toHaveAccessibleName(/app\.ts/);
  await fireEvent.keyDown(document.activeElement!, { key: "ArrowLeft" });
  expect(document.activeElement).toBe(src);
  await fireEvent.keyDown(src, { key: "End" });
  expect(document.activeElement).toHaveAccessibleName("README.md");
});

test("Changes lists what changed since the last commit and shows a file's diff", async () => {
  const patch = ["diff --git a/src/app.ts b/src/app.ts", "--- a/src/app.ts", "+++ b/src/app.ts", "@@ -1,2 +1,2 @@", " const a = 1;", "-const b = 2;", "+const b = 3;", ""].join("\n");
  const calls = folder({ git_diff: () => ({ patch, binary: false, tooLarge: false }) });
  render(SessionPage);

  const tab = await screen.findByRole("tab", { name: "Changes, 1 file" });
  await fireEvent.click(tab);
  expect(localStorage.getItem("oc-session-side-tab")).toBe("changes");
  const list = await screen.findByRole("list", { name: "Uncommitted changes" });
  expect(within(list).getByText("+3 −1")).toBeInTheDocument();
  expect(screen.getByText("since abc1234")).toBeInTheDocument();

  await fireEvent.click(within(list).getByRole("button", { name: "src/app.ts, Modified, 3 lines added, 1 removed" }));
  expect(await screen.findByText("const b = 3;")).toBeInTheDocument();
  expect(calls.calls("git_diff")).toEqual([{ id: "s1", path: "src/app.ts", oldPath: null, untracked: false }]);
  expect(screen.getByText("const b = 2;").closest(".cl")).toHaveClass("del");
  // The viewer can also show the whole file, since the file still exists.
  expect(screen.getByRole("button", { name: "File" })).toHaveAttribute("aria-pressed", "false");
});

test("a file shows in its language's colors, names the language, and long lines can wrap", async () => {
  vi.mocked(highlight).mockImplementation(async (lines, _lang, show) => {
    show(lines.map((text) => [tok("#", "var(--syn-func)"), tok(text.slice(1))]));
  });
  folder({ folder_read: () => ({ text: "# Notes\n", size: 8, binary: false, tooLarge: false }) });
  render(SessionPage);
  const tree = await screen.findByRole("tree");
  await fireEvent.click(within(tree).getByRole("treeitem", { name: /README\.md/ }));

  const viewer = await screen.findByRole("region", { name: "README.md, read-only" });
  const mark = await within(viewer).findByText("#");
  expect(mark.getAttribute("style")).toContain("var(--syn-func)");
  expect(mark.closest(".tx")).toHaveTextContent("# Notes");
  expect(highlight).toHaveBeenCalledWith(["# Notes"], "markdown", expect.any(Function));
  expect(screen.getByText("Markdown")).toBeInTheDocument();

  const wrap = screen.getByRole("button", { name: "Wrap long lines" });
  expect(wrap).toHaveAttribute("aria-pressed", "false");
  await fireEvent.click(wrap);
  expect(wrap).toHaveAttribute("aria-pressed", "true");
  expect(mark.closest(".code")).toHaveClass("wrap");
  expect(localStorage.getItem("oc-viewer-wrap")).toBe("1");
});

test("a diff colors kept and added lines as the new file and removed lines as the old one", async () => {
  const patch = ["@@ -1,2 +1,2 @@", " const a = 1;", "-const b = 2;", "+const b = 3;", ""].join("\n");
  const sides: string[][] = [];
  vi.mocked(highlight).mockImplementation(async (lines, _lang, show) => {
    const color = sides.length === 0 ? "var(--syn-string)" : "var(--syn-tag)";
    sides.push(lines);
    show(lines.map((text) => [tok(text, color)]));
  });
  folder({ git_diff: () => ({ patch, binary: false, tooLarge: false }) });
  render(SessionPage);
  await fireEvent.click(await screen.findByRole("tab", { name: "Changes, 1 file" }));
  await fireEvent.click(await screen.findByRole("button", { name: /^src\/app\.ts, Modified/ }));

  const viewer = await screen.findByRole("region", { name: "src/app.ts, read-only" });
  await within(viewer).findByText("const b = 3;");
  expect(sides).toEqual([
    ["const a = 1;", "const b = 3;"],
    ["const a = 1;", "const b = 2;"],
  ]);
  const color = (text: string) => within(viewer).getByText(text).getAttribute("style");
  expect(color("const a = 1;")).toContain("var(--syn-string)");
  expect(color("const b = 3;")).toContain("var(--syn-string)");
  expect(color("const b = 2;")).toContain("var(--syn-tag)");
  expect(screen.getByText("TypeScript")).toBeInTheDocument();
});

const PICTURES: FolderEntry[] = [file("icon.svg"), file("huge.pdf"), file("logo.png"), file("spec.pdf")];

function pictures(extra: Record<string, (args: Record<string, unknown> | undefined) => unknown> = {}) {
  return folder(
    {
      folder_list: (a) => ({ entries: a?.dir === "" ? PICTURES : [], truncated: false }),
      folder_read_bytes: (a) => (a?.path === "huge.pdf" ? new Error(MEDIA_TOO_LARGE) : new Uint8Array([1, 2, 3]).buffer),
      ...extra,
    },
    gitStatus(),
  );
}

async function openFile(name: string) {
  const tree = await screen.findByRole("tree");
  await fireEvent.click(within(tree).getByRole("treeitem", { name: new RegExp(`^${name.replace(".", "\\.")}`) }));
  return screen.findByRole("region", { name: `${name}, read-only` });
}

test("a picture opens as itself with its size, and zooms in from fitted and back", async () => {
  const calls = pictures();
  render(SessionPage);
  const viewer = await openFile("logo.png");
  const img = await within(viewer).findByRole("img", { name: "logo.png" });
  expect(img).toHaveAttribute("src", "blob:picture");
  expect(calls.calls("folder_read_bytes")).toEqual([{ id: "s1", path: "logo.png" }]);
  expect(calls.calls("folder_read")).toEqual([]);

  // Drawn fitted at its own 64 px width.
  for (const [key, value] of [["naturalWidth", 64], ["naturalHeight", 32], ["clientWidth", 64]] as const) Object.defineProperty(img, key, { value });
  await fireEvent.load(img);
  expect(screen.getByText("PNG · 64 × 32")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Fit to the panel" })).toHaveAttribute("aria-pressed", "true");

  await fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
  const zoomed = screen.getByRole("button", { name: "Fit to the panel, now 125%" });
  expect(zoomed).toHaveTextContent("125%");
  expect(img.style.width).toBe("80px");
  await fireEvent.click(zoomed);
  expect(img.style.width).toBe("");
  expect(screen.getByRole("button", { name: "Fit to the panel" })).toHaveTextContent("Fit");
});

test("an SVG opens as a picture and shows its code on request", async () => {
  const calls = pictures({ folder_read: () => ({ text: "<svg/>\n", size: 7, binary: false, tooLarge: false }) });
  render(SessionPage);
  const viewer = await openFile("icon.svg");
  expect(await within(viewer).findByRole("img", { name: "icon.svg" })).toBeInTheDocument();
  const preview = screen.getByRole("button", { name: "Preview" });
  expect(preview).toHaveAttribute("aria-pressed", "true");

  await fireEvent.click(screen.getByRole("button", { name: "Code" }));
  expect(await within(viewer).findByText("<svg/>")).toBeInTheDocument();
  expect(calls.calls("folder_read")).toEqual([{ id: "s1", path: "icon.svg" }]);
  expect(screen.getByText("XML")).toBeInTheDocument();

  await fireEvent.click(preview);
  expect(await within(viewer).findByRole("img", { name: "icon.svg" })).toBeInTheDocument();
});

test("a PDF lays out every page, counts them, and draws the ones in view", async () => {
  const page = {
    getViewport: ({ scale }: { scale: number }) => ({ width: 612 * scale, height: 792 * scale }),
    render: vi.fn(() => ({ promise: Promise.resolve(), cancel() {} })),
    streamTextContent: vi.fn(),
  };
  pdf.getDocument.mockReturnValue({ promise: Promise.resolve({ numPages: 2, getPage: async () => page }), destroy: vi.fn(async () => undefined) });
  // Every watched page counts as in view.
  const real = globalThis.IntersectionObserver;
  globalThis.IntersectionObserver = class {
    constructor(private seen: (entries: { target: Element; isIntersecting: boolean }[]) => void) {}
    observe(target: Element) {
      this.seen([{ target, isIntersecting: true }]);
    }
    unobserve() {}
    disconnect() {}
  } as unknown as typeof IntersectionObserver;
  try {
    pictures();
    render(SessionPage);
    const viewer = await openFile("spec.pdf");
    const pages = await within(viewer).findAllByRole("group", { name: /^Page \d of 2$/ });
    expect(pages).toHaveLength(2);
    expect(screen.getByText("PDF · 2 pages")).toBeInTheDocument();
    expect(pdf.getDocument).toHaveBeenCalledWith({ data: new Uint8Array([1, 2, 3]) });
    await vi.waitFor(() => expect(pages[0].querySelector("canvas")).not.toBeNull());
    expect(page.render).toHaveBeenCalledTimes(2);
  } finally {
    globalThis.IntersectionObserver = real;
  }
});

test("a PDF over 50 MB or behind a password says so", async () => {
  const locked = Object.assign(new Error("No password given"), { name: "PasswordException" });
  pdf.getDocument.mockImplementation(() => ({ promise: Promise.reject(locked), destroy: vi.fn(async () => undefined) }));
  pictures();
  render(SessionPage);
  const huge = await openFile("huge.pdf");
  expect(await within(huge).findByText("huge.pdf is larger than 50 MB, so it is not shown here.")).toBeInTheDocument();
  expect(within(huge).queryByRole("button", { name: "Try again" })).toBeNull();

  const spec = await openFile("spec.pdf");
  expect(await within(spec).findByText("spec.pdf could not be read: it is protected with a password.")).toBeInTheDocument();
});

test("a folder outside git still has Files, and Changes says why it is empty", async () => {
  folder({}, gitStatus({ repo: false, branch: null, head: null, upstream: null }));
  render(SessionPage);
  await fireEvent.click(await screen.findByRole("tab", { name: "Changes" }));
  expect(await screen.findByText("uninote is not a git repository.")).toBeInTheDocument();
});

test("the side tabs move with the arrow keys and the choice is remembered", async () => {
  folder();
  const { unmount } = render(SessionPage);
  const files = await screen.findByRole("tab", { name: "Files" });
  files.focus();
  await fireEvent.keyDown(files, { key: "ArrowLeft" });
  const details = screen.getByRole("tab", { name: "Details" });
  expect(details).toHaveAttribute("aria-selected", "true");
  expect(document.activeElement).toBe(details);
  expect(screen.getByRole("tabpanel", { name: "Details" })).toHaveTextContent("Process");
  unmount();

  render(SessionPage);
  expect(await screen.findByRole("tab", { name: "Details" })).toHaveAttribute("aria-selected", "true");
});

test("a branch switch waits while the CLI works, and asks first once it can", async () => {
  const running = session({ status: "running" });
  app.sessions = [running];
  folder();
  app.sessions = [running];
  const { unmount } = render(SessionPage);
  await fireEvent.click(await screen.findByRole("tab", { name: /Branch/ }));
  expect(await screen.findByText("Claude Code is working in this folder. Stop it or mark it done before switching branches.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Switch to feature" })).toBeDisabled();
  unmount();

  const calls = folder({ git_switch: () => gitStatus({ branch: "feature" }) }, gitStatus({ changes: [change("notes.md", { code: "?" })] }));
  render(SessionPage);
  const button = await screen.findByRole("button", { name: "Switch to feature" });
  expect(button).toBeEnabled();
  expect(screen.getByText("Tracks origin/main · up to date")).toBeInTheDocument();
  await fireEvent.click(button);
  const dialog = await screen.findByRole("dialog", { name: "Switch to feature?" });
  await fireEvent.click(within(dialog).getByRole("button", { name: "Switch to feature" }));
  expect(calls.calls("git_switch")).toEqual([{ id: "s1", branch: "feature" }]);
});

test("tracked changes block a switch", async () => {
  folder();
  render(SessionPage);
  await fireEvent.click(await screen.findByRole("tab", { name: /Branch/ }));
  expect(await screen.findByText("Commit or stash the changes in this folder before switching branches.")).toBeInTheDocument();
  expect(await screen.findByRole("button", { name: "Switch to feature" })).toBeDisabled();
});

test("searching by name and by contents, and a text hit opens the file at its line", async () => {
  const calls = folder({
    folder_find: (a) =>
      a?.contents
        ? { matches: [{ path: "src/app.ts", line: 2, text: "const b = 2;" }], truncated: false }
        : { matches: [{ path: "src/app.ts", line: null, text: null }], truncated: false },
    folder_read: () => ({ text: "const a = 1;\nconst b = 2;\n", size: 26, binary: false, tooLarge: false }),
  });
  render(SessionPage);
  const find = await screen.findByRole("searchbox", { name: "Find in uninote" });
  await fireEvent.input(find, { target: { value: "app" } });
  const results = await screen.findByRole("list", { name: "Search results" });
  expect(within(results).getByRole("button", { name: /app\.ts/ })).toBeInTheDocument();
  expect(calls.calls("folder_find")).toEqual([{ id: "s1", query: "app", contents: false }]);

  await fireEvent.click(screen.getByRole("button", { name: "Contents" }));
  const line = await screen.findByRole("button", { name: "src/app.ts, line 2: const b = 2;" });
  await fireEvent.click(line);
  const viewer = await screen.findByRole("region", { name: "src/app.ts, read-only" });
  const hit = await within(viewer).findByText("const b = 2;");
  expect(hit.closest(".cl")).toHaveClass("hit");

  await fireEvent.keyDown(find, { key: "Escape" });
  expect(await screen.findByRole("tree")).toBeInTheDocument();
});

test("the panel stays beside the terminal, hides and shows from the header, and stays hidden next time", async () => {
  folder();
  const { unmount } = render(SessionPage);
  const toggle = await screen.findByRole("button", { name: "Files and details" });
  const side = document.getElementById("session-side")!;
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(toggle).toHaveAttribute("title", "Hide files and details");
  expect(side).toBeVisible();

  await fireEvent.click(toggle);
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(toggle).toHaveAttribute("title", "Show files and details");
  expect(side).not.toBeVisible();
  expect(side.closest(".detail-body")).toHaveClass("solo");
  expect(localStorage.getItem("oc-session-side-hidden")).toBe("1");
  unmount();

  // Hidden, the panel's git status is not read until it shows again.
  const again = folder();
  render(SessionPage);
  const hidden = await screen.findByRole("button", { name: "Files and details" });
  expect(hidden).toHaveAttribute("aria-expanded", "false");
  expect(again.calls("git_status")).toEqual([]);
  await fireEvent.click(hidden);
  expect(document.getElementById("session-side")).toBeVisible();
  expect(localStorage.getItem("oc-session-side-hidden")).toBeNull();
  expect(again.calls("git_status")).toEqual([{ id: "s1" }]);
});

test("below 720 the panel opens over the terminal and closes with Escape or an opened file", async () => {
  localStorage.setItem("oc-session-side-hidden", "1");
  narrowWindow();
  folder({ folder_read: () => ({ text: "const a = 1;\n", size: 13, binary: false, tooLarge: false }) });
  render(SessionPage);
  const toggle = await screen.findByRole("button", { name: "Files and details" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");

  await fireEvent.click(toggle);
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  // Opening it here is for this moment only; the wide window's choice stays as it was.
  expect(localStorage.getItem("oc-session-side-hidden")).toBe("1");
  const files = screen.getByRole("tab", { name: "Files" });
  files.focus();
  await fireEvent.keyDown(files, { key: "Escape" });
  expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(document.activeElement).toBe(toggle);

  await fireEvent.click(toggle);
  const tree = await screen.findByRole("tree");
  await fireEvent.click(within(tree).getByRole("treeitem", { name: /README\.md/ }));
  expect(await screen.findByText("const a = 1;")).toBeInTheDocument();
  expect(toggle).toHaveAttribute("aria-expanded", "false");
});
