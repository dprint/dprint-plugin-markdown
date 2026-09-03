// Renders the documents of a json array with commonmark.js, writing them out in
// the shape of the spec's examples for `tests/commonmark_spec_check.rs` to read.
// Run it with `deno run -A scripts/spec_fuzz/render.ts <inputs.json> <cases.json>`.

import * as commonmark from "npm:commonmark@0.31.2";

const [input, output] = Deno.args;
if (!input || !output) {
  console.error("usage: render.ts <inputs.json> <cases.json>");
  Deno.exit(1);
}
const documents: string[] = JSON.parse(await Deno.readTextFile(input));
const parser = new commonmark.Parser();
const renderer = new commonmark.HtmlRenderer();
const cases = documents.map((markdown, index) => ({
  markdown,
  html: renderer.render(parser.parse(markdown)),
  example: index + 1,
  section: "fuzz",
}));
await Deno.writeTextFile(output, JSON.stringify(cases));
