// Groups the failures of a spec check report by what differs first, so that
// thousands of them can be read as the handful of causes they have. Run it with
// `deno run -A scripts/spec_fuzz/classify.ts <report.txt> parse|format [shown]`.
//
// `parse` reads the examples whose parse differs from the expected html,
// `format` the ones that parsed as expected but whose formatted output doesn't
// read back the same way. `shown` is how many examples of each group to print
// (one by default).
//
// What commonmark.js does differently from the spec, and what the formatter
// does on purpose, is grouped under a name rather than shown: trailing
// whitespace on a line of an html block, `'` written as `&#x27;` in a
// destination, an extension of the formatter (a definition list, front matter),
// and whitespace outside of code, which the formatter decides for itself.

interface Example {
  markdown: string;
  formatted: string | undefined;
}

const [path, kind, shownText = "1"] = Deno.args;
if (!path || (kind !== "parse" && kind !== "format")) {
  console.error("usage: classify.ts <report.txt> parse|format [shown]");
  Deno.exit(1);
}
const shown = Number(shownText);
const entries = (await Deno.readTextFile(path)).split(/^==== /m).slice(1);
const groups = new Map<string, { expected: string; actual: string; examples: Example[] }>();
for (const entry of entries) {
  const head = entry.split("\n")[0];
  if (kind === "parse" && !head.includes("parse:FAIL")) {
    continue;
  }
  if (kind === "format" && !(head.includes("parse:ok") && head.includes("format:FAIL"))) {
    continue;
  }
  const { expected, actual, example } = classify(entry, kind);
  const key = JSON.stringify([expected, actual]);
  const group = groups.get(key) ?? { expected, actual, examples: [] };
  group.examples.push(example);
  groups.set(key, group);
}

const failures = [...groups.values()].reduce((count, group) => count + group.examples.length, 0);
console.log(`${entries.length} entries, ${failures} of them ${kind} failures in ${groups.size} groups`);
for (const group of [...groups.values()].sort((a, b) => b.examples.length - a.examples.length)) {
  console.log(
    `==== ${group.examples.length} | expected: ${JSON.stringify(group.expected)} | actual: ${
      JSON.stringify(group.actual)
    }`,
  );
  if (!group.actual) {
    continue; // a named group, which there is nothing to look at in
  }
  for (const example of group.examples.slice(0, shown)) {
    console.log(`   markdown:  ${JSON.stringify(example.markdown)}`);
    if (example.formatted !== undefined) {
      console.log(`   formatted: ${JSON.stringify(example.formatted)}`);
    }
  }
}

function classify(entry: string, kind: string): { expected: string; actual: string; example: Example } {
  const markdown = field(entry, "markdown", "expected") ?? "";
  let expected = field(entry, "expected", "parsed|formatted markdown");
  let actual;
  let formatted;
  if (kind === "parse") {
    actual = field(entry, "parsed", "formatted markdown");
  } else {
    actual = field(entry, "formatted html", "note");
    formatted = field(entry, "formatted markdown", "formatted html");
  }
  const example = { markdown, formatted };
  if (expected === undefined || actual === undefined) {
    return { expected: "<incomplete entry>", actual: "", example };
  }
  if (actual.includes("extension:")) {
    return { expected: "extension of the formatter", actual: "", example };
  }
  expected = expected.replaceAll("&#x27;", "'");
  actual = actual.replaceAll("&#x27;", "'");
  let expectedParts;
  let actualParts;
  if (kind === "format") {
    expected = loosen(expected);
    actual = loosen(actual);
    if (expected === actual) {
      return { expected: "whitespace only", actual: "", example };
    }
    expectedParts = expected.split(/(?=<)/);
    actualParts = actual.split(/(?=<)/);
  } else {
    if (stripLines(expected) === stripLines(actual)) {
      return { expected: "trailing whitespace only", actual: "", example };
    }
    expectedParts = expected.split("\n");
    actualParts = actual.split("\n");
  }
  for (let i = 0; i < Math.min(expectedParts.length, actualParts.length); i++) {
    if (expectedParts[i] !== actualParts[i]) {
      return { expected: expectedParts[i].slice(0, 70), actual: actualParts[i].slice(0, 70), example };
    }
  }
  return { expected: "<length differs>", actual: String(expectedParts.length - actualParts.length), example };
}

function field(entry: string, name: string, nextNames: string): string | undefined {
  const match = entry.match(new RegExp(`--- ${name}\\n([\\s\\S]*?)(?=--- (?:${nextNames})\\n|$)`));
  return match?.[1];
}

function stripLines(text: string): string {
  return text.split("\n").map((line) => line.replace(/[ \t]+$/, "")).join("\n");
}

/** The html with the whitespace outside of code collapsed, the way the spec check compares formatted output. */
function loosen(html: string): string {
  let out = "";
  let rest = html;
  while (true) {
    const start = rest.indexOf("<pre>");
    if (start < 0) {
      return out + collapse(rest);
    }
    out += collapse(rest.slice(0, start));
    const close = rest.indexOf("</pre>", start);
    const end = close < 0 ? rest.length : close + 6;
    out += stripLines(rest.slice(start, end));
    rest = rest.slice(end);
  }
}

function collapse(html: string): string {
  let out = "";
  let inSpace = false;
  for (const c of html) {
    if (/\s/.test(c)) {
      inSpace = true;
      continue;
    }
    if (inSpace && !out.endsWith(">") && c !== "<") {
      out += " ";
    }
    inSpace = false;
    out += c;
  }
  return out;
}
