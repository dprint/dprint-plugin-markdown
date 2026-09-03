# Spec fuzz

Compares the parser and the formatter with commonmark.js on random markdown, through the spec check in `tests/commonmark_spec_check.rs`. The spec's own examples cover what the spec chose to show; random documents cover what it didn't, which is where the bugs were.

## Running

Random documents are written out, rendered by commonmark.js (pinned to the spec version the check is written against) into the shape of the spec's examples, and read by the spec check the same way the spec is. `target` is a good place for the files, since git ignores it.

```sh
mkdir -p target/spec_fuzz
deno run -A scripts/spec_fuzz/generate.ts 1 20000 target/spec_fuzz/inputs.json
deno run -A scripts/spec_fuzz/render.ts target/spec_fuzz/inputs.json target/spec_fuzz/cases.json
SPEC_PRESERVE_CODE=1 COMMONMARK_SPEC=target/spec_fuzz/cases.json SPEC_REPORT=target/spec_fuzz/report.txt \
  cargo test --release --test commonmark_spec_check -- --nocapture
deno run -A scripts/spec_fuzz/classify.ts target/spec_fuzz/report.txt parse
deno run -A scripts/spec_fuzz/classify.ts target/spec_fuzz/report.txt format
```

The first argument of `generate.ts` is the seed, so a run can be repeated. Pass `inline` as its fourth argument for documents that are one paragraph of emphasis and links each, which is what exercises how a decoration's delimiters are chosen.

`SPEC_PRESERVE_CODE=1` turns on the code block options that would otherwise account for most of the formatter's differences; leave it out to see those too.

## Reading the results

`classify.ts` groups the failures by the first line (for the parser) or the first tag (for the formatter) that differs, and prints one example of each group. The groups that are known and not worth reading have names instead:

- `trailing whitespace only`: the parser drops trailing whitespace from the lines of an html block.
- `whitespace only`: the formatter decides the whitespace between words itself, and the spec check compares formatted output with that collapsed.
- `extension of the formatter`: the document held something the formatter reads as an extension (a definition list, front matter).

Some of what commonmark.js does isn't what the spec says, and shows up as a parse failure that isn't one: it keeps a tab at the end of a line, doesn't read a tab after a definition's colon, strips a non-breaking space from the edge of a line, and writes an empty `<p></p>` for a paragraph that held nothing but a definition. The generator avoids the first three; the last has to be read past.
