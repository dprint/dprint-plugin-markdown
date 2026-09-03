// Renders the documents of a json array with commonmark.js, writing them out in
// the shape of the spec's examples for `tests/commonmark_spec_check.rs` to read.
//
//     node render.js <inputs.json> <cases.json>
const fs = require("fs");
const commonmark = require("commonmark");

const parser = new commonmark.Parser();
const renderer = new commonmark.HtmlRenderer();
const documents = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const cases = documents.map((markdown, index) => ({
  markdown,
  html: renderer.render(parser.parse(markdown)),
  example: index + 1,
  section: "fuzz",
}));
fs.writeFileSync(process.argv[3], JSON.stringify(cases));
