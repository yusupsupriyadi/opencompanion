// Syntax colors for the file viewer. Shiki reads the TextMate grammars VS Code uses; the theme's colors are CSS
// variables (`--syn-*` in app.css), so the palette sits next to the terminal colors in one place.
import type { GrammarState, HighlighterCore, LanguageInput, ThemeRegistration, ThemedToken } from "shiki/core";

/** One run of text in one style. `color` is empty for plain text, which takes the viewer's own color. */
export type Token = { text: string; color: string; italic: boolean; bold: boolean };

// Only these grammars ship with the app: the languages a coding project usually holds. All of Shiki's would add about
// 9 MB for files that are rarely opened here, and anything not listed still shows, as plain text.
const GRAMMARS = {
  typescript: () => import("shiki/langs/typescript.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  jsx: () => import("shiki/langs/jsx.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  jsonc: () => import("shiki/langs/jsonc.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  scss: () => import("shiki/langs/scss.mjs"),
  svelte: () => import("shiki/langs/svelte.mjs"),
  vue: () => import("shiki/langs/vue.mjs"),
  astro: () => import("shiki/langs/astro.mjs"),
  rust: () => import("shiki/langs/rust.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  java: () => import("shiki/langs/java.mjs"),
  kotlin: () => import("shiki/langs/kotlin.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  csharp: () => import("shiki/langs/csharp.mjs"),
  php: () => import("shiki/langs/php.mjs"),
  ruby: () => import("shiki/langs/ruby.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
  dart: () => import("shiki/langs/dart.mjs"),
  lua: () => import("shiki/langs/lua.mjs"),
  shellscript: () => import("shiki/langs/shellscript.mjs"),
  powershell: () => import("shiki/langs/powershell.mjs"),
  bat: () => import("shiki/langs/bat.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  graphql: () => import("shiki/langs/graphql.mjs"),
  xml: () => import("shiki/langs/xml.mjs"),
  docker: () => import("shiki/langs/docker.mjs"),
  make: () => import("shiki/langs/make.mjs"),
  ini: () => import("shiki/langs/ini.mjs"),
  dotenv: () => import("shiki/langs/dotenv.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
} satisfies Record<string, LanguageInput>;

export type Lang = keyof typeof GRAMMARS;
export const LANGS = Object.keys(GRAMMARS) as Lang[];

/** Shown in the viewer's bar, so it is clear what the colors follow. */
export const LANG_NAME: Record<Lang, string> = {
  typescript: "TypeScript",
  tsx: "TSX",
  javascript: "JavaScript",
  jsx: "JSX",
  json: "JSON",
  jsonc: "JSON with comments",
  markdown: "Markdown",
  html: "HTML",
  css: "CSS",
  scss: "SCSS",
  svelte: "Svelte",
  vue: "Vue",
  astro: "Astro",
  rust: "Rust",
  toml: "TOML",
  yaml: "YAML",
  python: "Python",
  go: "Go",
  java: "Java",
  kotlin: "Kotlin",
  c: "C",
  cpp: "C++",
  csharp: "C#",
  php: "PHP",
  ruby: "Ruby",
  swift: "Swift",
  dart: "Dart",
  lua: "Lua",
  shellscript: "Shell",
  powershell: "PowerShell",
  bat: "Batch",
  sql: "SQL",
  graphql: "GraphQL",
  xml: "XML",
  docker: "Dockerfile",
  make: "Makefile",
  ini: "INI",
  dotenv: ".env",
  diff: "Diff",
};

const BY_EXT = new Map<string, Lang>([
  ...(["ts", "mts", "cts"].map((e) => [e, "typescript"]) as [string, Lang][]),
  ["tsx", "tsx"],
  ...(["js", "mjs", "cjs"].map((e) => [e, "javascript"]) as [string, Lang][]),
  ["jsx", "jsx"],
  ["json", "json"],
  ["jsonc", "jsonc"],
  ["json5", "jsonc"],
  ["md", "markdown"],
  ["markdown", "markdown"],
  ["html", "html"],
  ["htm", "html"],
  ["css", "css"],
  ["scss", "scss"],
  ["svelte", "svelte"],
  ["vue", "vue"],
  ["astro", "astro"],
  ["rs", "rust"],
  ["toml", "toml"],
  ["yaml", "yaml"],
  ["yml", "yaml"],
  ["py", "python"],
  ["pyi", "python"],
  ["go", "go"],
  ["java", "java"],
  ["kt", "kotlin"],
  ["kts", "kotlin"],
  ["c", "c"],
  ["h", "c"],
  ...(["cpp", "cc", "cxx", "hpp", "hh", "hxx"].map((e) => [e, "cpp"]) as [string, Lang][]),
  ["cs", "csharp"],
  ["php", "php"],
  ["rb", "ruby"],
  ["swift", "swift"],
  ["dart", "dart"],
  ["lua", "lua"],
  ...(["sh", "bash", "zsh"].map((e) => [e, "shellscript"]) as [string, Lang][]),
  ...(["ps1", "psm1", "psd1"].map((e) => [e, "powershell"]) as [string, Lang][]),
  ["bat", "bat"],
  ["cmd", "bat"],
  ["sql", "sql"],
  ["graphql", "graphql"],
  ["gql", "graphql"],
  ...(["xml", "svg", "plist", "csproj", "xaml"].map((e) => [e, "xml"]) as [string, Lang][]),
  ["ini", "ini"],
  ["cfg", "ini"],
  ["diff", "diff"],
  ["patch", "diff"],
]);

// Files known by their whole name, before the extension is looked at.
const BY_NAME = new Map<string, Lang>([
  ["dockerfile", "docker"],
  ["makefile", "make"],
  ["gnumakefile", "make"],
  ["cargo.lock", "toml"],
  ["bun.lock", "jsonc"],
  ["tsconfig.json", "jsonc"],
  ["jsconfig.json", "jsonc"],
  [".bashrc", "shellscript"],
  [".zshrc", "shellscript"],
  [".profile", "shellscript"],
  [".editorconfig", "ini"],
  [".gitconfig", "ini"],
  [".npmrc", "ini"],
]);

/** The grammar for a file, from its name; null when the file is shown as plain text. */
export function langFor(path: string): Lang | null {
  const name = (path.split(/[\\/]/).pop() ?? "").toLowerCase();
  const known = BY_NAME.get(name);
  if (known) return known;
  if (name === ".env" || name.startsWith(".env.")) return "dotenv";
  if (name.startsWith("dockerfile.") || name.endsWith(".dockerfile")) return "docker";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? (BY_EXT.get(name.slice(dot + 1)) ?? null) : null;
}

const color = (name: string) => `var(--syn-${name})`;

// A TextMate scope picks the longest selector that matches it, so `keyword.operator` (plain) wins over `keyword`.
const THEME: ThemeRegistration = {
  name: "meadow",
  type: "dark",
  fg: color("text"),
  bg: "var(--term-bg)",
  settings: [
    { scope: ["comment", "punctuation.definition.comment", "markup.quote"], settings: { foreground: color("comment"), fontStyle: "italic" } },
    {
      scope: ["keyword", "storage", "variable.language", "keyword.operator.new", "keyword.operator.expression", "punctuation.section.embedded", "punctuation.definition.template-expression"],
      settings: { foreground: color("keyword") },
    },
    { scope: ["keyword.operator"], settings: { foreground: color("text") } },
    { scope: ["string", "markup.inline.raw", "markup.fenced_code", "markup.inserted"], settings: { foreground: color("string") } },
    {
      scope: ["constant", "support.constant", "string.regexp", "entity.other.attribute-name", "punctuation.definition.list", "markup.changed"],
      settings: { foreground: color("number") },
    },
    {
      scope: ["entity.name.function", "support.function", "variable.function", "meta.decorator", "support.type.property-name", "markup.underline.link", "string.other.link"],
      settings: { foreground: color("func") },
    },
    {
      scope: ["entity.name.type", "entity.name.class", "entity.name.namespace", "entity.other.inherited-class", "support.type", "support.class", "storage.type.primitive"],
      settings: { foreground: color("type") },
    },
    { scope: ["entity.name.tag", "markup.deleted"], settings: { foreground: color("tag") } },
    { scope: ["markup.heading", "entity.name.section"], settings: { foreground: color("func"), fontStyle: "bold" } },
    { scope: ["markup.bold"], settings: { fontStyle: "bold" } },
    { scope: ["markup.italic"], settings: { fontStyle: "italic" } },
  ],
};

let starting: Promise<HighlighterCore> | undefined;
const loading = new Map<Lang, Promise<void>>();

// Shiki is loaded the first time a file is opened, not with the app.
function highlighter(): Promise<HighlighterCore> {
  starting ??= (async () => {
    const [{ createHighlighterCore }, { createOnigurumaEngine }] = await Promise.all([import("shiki/core"), import("shiki/engine/oniguruma")]);
    return createHighlighterCore({ themes: [THEME], langs: [], engine: createOnigurumaEngine(import("shiki/wasm")) });
  })().catch((e) => {
    starting = undefined;
    throw e;
  });
  return starting;
}

async function ready(lang: Lang): Promise<HighlighterCore> {
  const hl = await highlighter();
  let load = loading.get(lang);
  if (!load) {
    load = hl.loadLanguage(GRAMMARS[lang]).catch((e) => {
      loading.delete(lang);
      throw e;
    });
    loading.set(lang, load);
  }
  await load;
  return hl;
}

/** Lines colored per slice; the page gets a turn between slices. */
const SLICE = 300;
/** A line longer than this (minified code) stays plain rather than holding the page up. */
const LONG_LINE = 2000;

function merge(line: ThemedToken[]): Token[] {
  const out: Token[] = [];
  for (const t of line) {
    const c = t.color === THEME.fg ? "" : (t.color ?? "");
    const italic = ((t.fontStyle ?? 0) & 1) !== 0;
    const bold = ((t.fontStyle ?? 0) & 2) !== 0;
    const last = out.at(-1);
    if (last && last.color === c && last.italic === italic && last.bold === bold) last.text += t.content;
    else out.push({ text: t.content, color: c, italic, bold });
  }
  return out;
}

/**
 * Colors `lines` a slice at a time, so a long file never freezes the page. `show` receives the lines colored so far:
 * once after the first slice, so the top of the file colors straight away, and once at the end. It returns false
 * when the viewer has moved on, which stops the work.
 */
export async function highlight(lines: string[], lang: Lang, show: (tokens: Token[][]) => boolean): Promise<void> {
  const hl = await ready(lang);
  const out: Token[][] = [];
  let state: GrammarState | undefined;
  for (let at = 0; at < lines.length; at += SLICE) {
    if (at > 0) await new Promise((r) => setTimeout(r, 0));
    const slice = lines.slice(at, at + SLICE);
    const tokens = hl.codeToTokensBase(slice.join("\n"), { lang, theme: THEME.name, grammarState: state, tokenizeMaxLineLength: LONG_LINE });
    state = hl.getLastGrammarState(tokens);
    for (let i = 0; i < slice.length; i++) out.push(merge(tokens[i] ?? []));
    const done = at + SLICE >= lines.length;
    if ((at === 0 || done) && !show(out)) return;
  }
}
