// Writes random markdown documents for `render.ts` to render, built out of the
// pieces markdown is written with. Run it with
// `deno run -A scripts/spec_fuzz/generate.ts <seed> <count> <out.json> [block|inline]`.
//
// `block` (the default) builds documents a line at a time out of a prefix (what
// makes a line an item, a quote, a heading) and a body, since that is the shape
// a block parser reads. `inline` builds one paragraph at a time out of the
// pieces emphasis and links are written with, which is what finds the trouble
// in how a decoration's delimiters are chosen.
//
// What the formatter reads as an extension is left out (definition lists, front
// matter), along with what commonmark.js reads differently from the spec: a tab
// at the end of a line, a tab after a definition's colon, and a non-breaking
// space at the edge of a line.

const PREFIXES = [
  "",
  "",
  "",
  "",
  " ",
  "  ",
  "   ",
  "    ",
  "\t",
  "- ",
  "* ",
  "+ ",
  "1. ",
  "2) ",
  "10. ",
  "> ",
  ">",
  "> > ",
  "# ",
  "## ",
  "###### ",
  "  - ",
  "    - ",
  "-",
  "1.",
  "[r1]: ",
  "  [r1]: /u \"t\"",
  "```",
  "````",
  "    ",
  "=",
  "==",
  "---",
  "***",
  "<div>",
  "</div>",
  "<!-- ",
  "<b>",
  "<pre>",
  "</pre>",
  "  > ",
  "- - ",
  "1. - ",
  "> - ",
];
const PIECES = [
  " ",
  " ",
  "  ",
  "\t",
  "---",
  "***",
  "___",
  "=",
  "===",
  "`",
  "``",
  "` `",
  "*",
  "**",
  "***",
  "_",
  "__",
  "\\",
  "\\*",
  "\\\\",
  "[",
  "]",
  "[r1]",
  "[r1][]",
  "[a][r1]",
  "![a](/u)",
  "[a](/u)",
  "[a](/u \"t\")",
  "[a](<b c>)",
  "(",
  ")",
  "<",
  ">",
  "<b>",
  "</b>",
  "<b c=\"d\">",
  "<!-- c -->",
  "<!---->",
  "<http://x.y>",
  "<a@b.c>",
  "&amp;",
  "&#35;",
  "&copy;",
  "&x;",
  "#",
  "\"",
  "'",
  "!",
  "-",
  "+",
  "1.",
  ".",
  ":",
  "。",
  "—",
  "“",
  "é",
  "*a*",
  "**a**",
  "_a_",
  "`a`",
  "a*",
  "*a",
  "a_",
  "_a",
  "-a",
  "a-",
  "foo",
  "bar",
  "baz",
  "qux",
  "x",
  "1",
  "22",
  "a1",
  "ﬁ",
];
const INLINE_PIECES = [
  "*",
  "*",
  "**",
  "**",
  "***",
  "****",
  "_",
  "_",
  "__",
  "___",
  " ",
  " ",
  " ",
  "a",
  "a",
  "b",
  "foo",
  "1",
  ".",
  ",",
  "!",
  "'",
  "\"",
  "(",
  ")",
  "[",
  "]",
  "。",
  "—",
  "é",
  "\\*",
  "\\_",
  "\\\\",
  "`",
  "``",
  "` `",
  "`a`",
  "[a](/u)",
  "[a](/u \"t\")",
  "[a](<b c>)",
  "[a](<b\\>c>)",
  "[a](/u(x))",
  "[a](/u 'x')",
  "[a](/u (x))",
  "[a](<>)",
  "[a]()",
  "[a](/u \"x\" )",
  "[a]( /u )",
  "[a][r1]",
  "[r1][]",
  "[r1]",
  "[a][r1",
  "[[a]](/u)",
  "![a](/u)",
  "![[a]](/u)",
  "[![a](/u)](/u2)",
  "![a][r1]",
  "<a href=\"x\">",
  "<a  b  = 'c'>",
  "<a b=c>",
  "<a/>",
  "<a >",
  "<a>",
  "</a >",
  "<?x?>",
  "<!x>",
  "<![CDATA[x]]>",
  "<!-- x -->",
  "<!--->",
  "<!-->",
  "<http://x.y>",
  "<a@b.c>",
  "&amp;",
  "&#35;",
  "&x;",
  "\n",
  "  \n",
  "\\\n",
];

const [seedText, countText, out, mode = "block"] = Deno.args;
if (!seedText || !countText || !out) {
  console.error("usage: generate.ts <seed> <count> <out.json> [block|inline]");
  Deno.exit(1);
}
const next = random(Number(seedText));
const build = mode === "inline" ? inlineDocument : blockDocument;
const documents: string[] = [];
for (let i = 0; i < Number(countText); i++) {
  let document = build();
  // front matter is an extension of the formatter, so never open with its fence
  if (document.startsWith("---") || document.startsWith("+++")) {
    document = " " + document;
  }
  documents.push(document);
}
await Deno.writeTextFile(out, JSON.stringify(documents));

function blockDocument(): string {
  const lines = [];
  for (let i = between(1, 8); i > 0; i--) {
    lines.push(next() > 0.15 ? blockLine() : "");
  }
  return lines.join("\n") + (next() > 0.2 ? "\n" : "");
}

function blockLine(): string {
  let body = "";
  for (let i = between(0, 6); i > 0; i--) {
    body += pick(PIECES);
  }
  const text = (pick(PREFIXES) + body).replace(/\t+$/, "").replace("[r1]: \t", "[r1]: ");
  // a definition list is an extension of the formatter
  if (/^\s*:(\s|$)/.test(text)) {
    return blockLine();
  }
  return text;
}

function inlineDocument(): string {
  let text = "";
  for (let i = between(1, 14); i > 0; i--) {
    text += pick(INLINE_PIECES);
  }
  text = text.split("\n").map((line) => line.replace(/\t+$/, "")).join("\n");
  if (next() < 0.3) {
    text += "\n\n[r1]: /u \"t\"";
  }
  return text + "\n";
}

function pick<T>(items: T[]): T {
  return items[Math.floor(next() * items.length)];
}

/** A whole number from `min` through `max`. */
function between(min: number, max: number): number {
  return min + Math.floor(next() * (max - min + 1));
}

/** A generator of numbers from 0 up to 1 that gives the same run for a seed. */
function random(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
