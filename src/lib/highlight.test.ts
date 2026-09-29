import { expect, test } from "vitest";
import { LANGS, highlight, langFor, type Lang, type Token } from "./highlight";

async function colors(lines: string[], lang: Lang) {
  let out: Token[][] = [];
  await highlight(lines, lang, (t) => {
    out = t.slice();
    return true;
  });
  return out;
}

test("a file's grammar comes from its name, and anything unknown stays plain", () => {
  expect(langFor("src/app.ts")).toBe("typescript");
  expect(langFor(String.raw`C:\Users\me\Project\README.MD`)).toBe("markdown");
  expect(langFor("docker/Dockerfile")).toBe("docker");
  expect(langFor("Dockerfile.dev")).toBe("docker");
  expect(langFor(".env.local")).toBe("dotenv");
  expect(langFor("tsconfig.json")).toBe("jsonc");
  expect(langFor("src-tauri/Cargo.lock")).toBe("toml");
  expect(langFor("notes.txt")).toBeNull();
  expect(langFor(".gitignore")).toBeNull();
  expect(langFor("constructor")).toBeNull();
});

test("tokens keep the text and carry the theme's colors", async () => {
  const [line] = await colors(['const greeting = "hi"; // say it'], "typescript");
  expect(line.map((t) => t.text).join("")).toBe('const greeting = "hi"; // say it');
  expect(line.find((t) => t.text === "const")?.color).toBe("var(--syn-keyword)");
  expect(line.find((t) => t.text.includes('"hi"'))?.color).toBe("var(--syn-string)");
  const comment = line.find((t) => t.text.includes("say it"));
  expect(comment).toMatchObject({ color: "var(--syn-comment)", italic: true });
  // Plain text takes the viewer's own color.
  expect(line.find((t) => t.text.includes("greeting"))?.color).toBe("");
});

test("a block comment carries on across slices, and the page hears once early and once at the end", async () => {
  const lines = Array.from({ length: 650 }, (_, i) => `let v${i} = ${i};`);
  lines[298] = "/* starts here";
  lines[301] = "ends here */";
  const seen: number[] = [];
  let last: Token[][] = [];
  await highlight(lines, "javascript", (t) => {
    seen.push(t.length);
    last = t;
    return true;
  });
  expect(seen).toEqual([300, 650]);
  expect(last[300][0].color).toBe("var(--syn-comment)");
  expect(last[302].find((t) => t.text === "let")?.color).toBe("var(--syn-keyword)");

  // Returning false stops the work after the first slice.
  const stopped: number[] = [];
  await highlight(lines, "javascript", (t) => (stopped.push(t.length), false));
  expect(stopped).toEqual([300]);
});

test("every bundled grammar loads under its own name", async () => {
  for (const lang of LANGS) {
    const [line] = await colors(["x = 1"], lang);
    expect(line.map((t) => t.text).join(""), lang).toBe("x = 1");
  }
}, 30_000);
