<script module lang="ts">
  import File from "phosphor-svelte/lib/File";
  import FileArchive from "phosphor-svelte/lib/FileArchive";
  import FileAudio from "phosphor-svelte/lib/FileAudio";
  import FileC from "phosphor-svelte/lib/FileC";
  import FileCSharp from "phosphor-svelte/lib/FileCSharp";
  import FileCode from "phosphor-svelte/lib/FileCode";
  import FileCpp from "phosphor-svelte/lib/FileCpp";
  import FileCss from "phosphor-svelte/lib/FileCss";
  import FileCsv from "phosphor-svelte/lib/FileCsv";
  import FileDoc from "phosphor-svelte/lib/FileDoc";
  import FileHtml from "phosphor-svelte/lib/FileHtml";
  import FileImage from "phosphor-svelte/lib/FileImage";
  import FileIni from "phosphor-svelte/lib/FileIni";
  import FileJpg from "phosphor-svelte/lib/FileJpg";
  import FileJs from "phosphor-svelte/lib/FileJs";
  import FileJsx from "phosphor-svelte/lib/FileJsx";
  import FileLock from "phosphor-svelte/lib/FileLock";
  import FileMd from "phosphor-svelte/lib/FileMd";
  import FilePdf from "phosphor-svelte/lib/FilePdf";
  import FilePng from "phosphor-svelte/lib/FilePng";
  import FilePpt from "phosphor-svelte/lib/FilePpt";
  import FilePy from "phosphor-svelte/lib/FilePy";
  import FileRs from "phosphor-svelte/lib/FileRs";
  import FileSql from "phosphor-svelte/lib/FileSql";
  import FileSvg from "phosphor-svelte/lib/FileSvg";
  import FileTs from "phosphor-svelte/lib/FileTs";
  import FileTsx from "phosphor-svelte/lib/FileTsx";
  import FileTxt from "phosphor-svelte/lib/FileTxt";
  import FileVideo from "phosphor-svelte/lib/FileVideo";
  import FileVue from "phosphor-svelte/lib/FileVue";
  import FileXls from "phosphor-svelte/lib/FileXls";

  type Icon = typeof File;

  const group = (icon: Icon, exts: string[]) => exts.map((e): [string, Icon] => [e, icon]);

  // Phosphor's own file glyphs, several with the type written on the page, so they match every other icon.
  const BY_EXT = new Map<string, Icon>([
    ["pdf", FilePdf],
    ...group(FileMd, ["md", "markdown", "mdx"]),
    ["png", FilePng],
    ...group(FileJpg, ["jpg", "jpeg"]),
    ["svg", FileSvg],
    ...group(FileImage, ["gif", "webp", "avif", "bmp", "ico", "tif", "tiff", "heic"]),
    ...group(FileTs, ["ts", "mts", "cts"]),
    ["tsx", FileTsx],
    ...group(FileJs, ["js", "mjs", "cjs"]),
    ["jsx", FileJsx],
    ["vue", FileVue],
    ...group(FileHtml, ["html", "htm"]),
    ...group(FileCss, ["css", "scss", "sass", "less"]),
    ...group(FilePy, ["py", "pyi"]),
    ["rs", FileRs],
    ...group(FileC, ["c", "h"]),
    ...group(FileCpp, ["cpp", "cc", "cxx", "hpp", "hh", "hxx"]),
    ["cs", FileCSharp],
    ["sql", FileSql],
    ["csv", FileCsv],
    ...group(FileIni, ["ini", "cfg", "conf", "toml", "env"]),
    ["lock", FileLock],
    ...group(FileTxt, ["txt", "log"]),
    ...group(FileDoc, ["doc", "docx", "odt", "rtf"]),
    ...group(FileXls, ["xls", "xlsx", "ods"]),
    ...group(FilePpt, ["ppt", "pptx", "odp"]),
    ...group(FileArchive, ["zip", "tar", "gz", "tgz", "7z", "rar", "bz2", "xz"]),
    ...group(FileAudio, ["mp3", "wav", "ogg", "flac", "m4a"]),
    ...group(FileVideo, ["mp4", "mov", "webm", "mkv", "avi"]),
    ...group(FileCode, [
      "svelte", "astro", "json", "jsonc", "json5", "yaml", "yml", "xml", "go", "java", "kt", "kts", "php", "rb", "swift",
      "dart", "lua", "sh", "bash", "zsh", "ps1", "psm1", "bat", "cmd", "graphql", "gql", "diff", "patch",
    ]),
  ]);

  const BY_NAME = new Map<string, Icon>([
    ["package-lock.json", FileLock],
    ["dockerfile", FileCode],
    ["makefile", FileCode],
    [".gitignore", FileIni],
    [".editorconfig", FileIni],
    [".npmrc", FileIni],
  ]);

  export type Tone = "red" | "orange" | "yellow" | "green" | "blue" | "purple";

  const tones = (tone: Tone, exts: string[]) => exts.map((e): [string, Tone] => [e, tone]);

  // The color most people already tie to the type (PDF red, JavaScript yellow, TypeScript blue), from the six
  // `--ft-*` tokens. Plain text, lock files, and unknown files keep the tab's own color.
  const CONFIG = ["json", "jsonc", "json5", "yaml", "yml", "toml", "ini", "cfg", "conf", "env"];
  const TONE = new Map<string, Tone>([
    ...tones("red", ["pdf", "rb", "graphql", "gql"]),
    ...tones("orange", ["html", "htm", "xml", "rs", "svelte", "astro", "java", "swift", "ppt", "pptx", "odp"]),
    ...tones("orange", ["zip", "tar", "gz", "tgz", "7z", "rar", "bz2", "xz"]),
    ...tones("yellow", ["js", "mjs", "cjs", "jsx", "sql", ...CONFIG]),
    ...tones("green", ["vue", "csv", "xls", "xlsx", "ods", "sh", "bash", "zsh", "bat", "cmd"]),
    ...tones("blue", ["md", "markdown", "mdx", "ts", "mts", "cts", "tsx", "css", "scss", "sass", "less", "py", "pyi"]),
    ...tones("blue", ["c", "h", "cpp", "cc", "cxx", "hpp", "hh", "hxx", "go", "dart", "lua", "ps1", "psm1", "doc", "docx", "odt", "rtf"]),
    ...tones("purple", ["png", "jpg", "jpeg", "svg", "gif", "webp", "avif", "bmp", "ico", "tif", "tiff", "heic"]),
    ...tones("purple", ["mp3", "wav", "ogg", "flac", "m4a", "mp4", "mov", "webm", "mkv", "avi", "cs", "kt", "kts", "php"]),
  ]);
  const TONE_BY_NAME = new Map<string, Tone>([
    ["dockerfile", "blue"],
    ["makefile", "orange"],
    [".gitignore", "yellow"],
    [".editorconfig", "yellow"],
    [".npmrc", "yellow"],
  ]);

  const fileName = (path: string) => (path.split(/[\\/]/).pop() ?? "").toLowerCase();
  const extension = (name: string) => {
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(dot + 1) : "";
  };

  /** The glyph for a file, from its name; a plain page when nothing more is known. */
  export function fileIcon(path: string): Icon {
    const name = fileName(path);
    const known = BY_NAME.get(name);
    if (known) return known;
    if (name === ".env" || name.startsWith(".env.")) return FileIni;
    return BY_EXT.get(extension(name)) ?? File;
  }

  /** The glyph's color, or null for the tab's own color. */
  export function fileTone(path: string): Tone | null {
    const name = fileName(path);
    if (name === "package-lock.json") return null;
    const known = TONE_BY_NAME.get(name);
    if (known) return known;
    if (name === ".env" || name.startsWith(".env.")) return "yellow";
    return TONE.get(extension(name)) ?? null;
  }
</script>

<script lang="ts">
  let { path, size = 16 }: { path: string; size?: number } = $props();
  const Glyph = $derived(fileIcon(path));
  const tone = $derived(fileTone(path));
</script>

<Glyph {size} class={tone ? `ft-${tone}` : undefined} aria-hidden="true" />
