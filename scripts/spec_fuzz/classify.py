#!/usr/bin/env python3
"""Groups the failures of a spec check report by what differs first, so that
thousands of them can be read as the handful of causes they have.

    classify.py <report.txt> parse|format [shown]

`parse` reads the examples whose parse differs from the expected html, `format`
the ones that parsed as expected but whose formatted output doesn't read back
the same way. `shown` is how many examples of each group to print (one by
default).

What commonmark.js does differently from the spec, and what the formatter does
on purpose, is grouped under a name rather than shown: trailing whitespace on a
line of an html block, `'` written as `&#x27;` in a destination, an extension
of the formatter (a definition list, front matter), and whitespace outside of
code, which the formatter decides for itself.
"""
import collections
import re
import sys


def main():
    path, kind = sys.argv[1], sys.argv[2]
    shown = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    entries = re.split(r"^==== ", open(path, encoding="utf-8").read(), flags=re.M)[1:]
    groups = collections.OrderedDict()
    for entry in entries:
        head = entry.split("\n")[0]
        if kind == "parse" and "parse:FAIL" not in head:
            continue
        if kind == "format" and ("parse:ok" not in head or "format:FAIL" not in head):
            continue
        key, example = classify(entry, kind)
        groups.setdefault(key, []).append(example)

    print(len(entries), "entries,", sum(len(g) for g in groups.values()), "of them", kind, "failures in", len(groups), "groups")
    for key, examples in sorted(groups.items(), key=lambda pair: -len(pair[1])):
        print("====", len(examples), "| expected:", repr(key[0]), "| actual:", repr(key[1]))
        if not key[1]:
            continue  # a named group, which there is nothing to look at in
        for markdown, formatted in examples[:shown]:
            print("   markdown: ", repr(markdown))
            if formatted is not None:
                print("   formatted:", repr(formatted))


def classify(entry, kind):
    markdown = field(entry, "markdown", "expected")
    expected = field(entry, "expected", "parsed|formatted markdown")
    if kind == "parse":
        actual = field(entry, "parsed", "formatted markdown")
        formatted = None
    else:
        actual = field(entry, "formatted html", "note")
        formatted = field(entry, "formatted markdown", "formatted html")
    example = (markdown, formatted)
    if actual is None or expected is None:
        return ("<incomplete entry>", ""), example
    if "extension:" in actual:
        return ("extension of the formatter", ""), example
    expected, actual = expected.replace("&#x27;", "'"), actual.replace("&#x27;", "'")
    if kind == "format":
        expected, actual = loosen(expected), loosen(actual)
        if expected == actual:
            return ("whitespace only", ""), example
        expected_parts, actual_parts = re.split(r"(?=<)", expected), re.split(r"(?=<)", actual)
    else:
        if strip_lines(expected) == strip_lines(actual):
            return ("trailing whitespace only", ""), example
        expected_parts, actual_parts = expected.split("\n"), actual.split("\n")
    for expected_part, actual_part in zip(expected_parts, actual_parts):
        if expected_part != actual_part:
            return (expected_part[:70], actual_part[:70]), example
    return ("<length differs>", str(len(expected_parts) - len(actual_parts))), example


def field(entry, name, next_names):
    match = re.search(r"--- " + name + r"\n(.*?)(?=--- (?:" + next_names + r")\n|\Z)", entry, re.S)
    return match.group(1) if match else None


def strip_lines(text):
    return "\n".join(line.rstrip(" \t") for line in text.split("\n"))


def loosen(html):
    """The html with the whitespace outside of code collapsed, the way the spec
    check compares formatted output."""
    out = ""
    rest = html
    while True:
        start = rest.find("<pre>")
        if start < 0:
            return out + collapse(rest)
        out += collapse(rest[:start])
        end = rest.find("</pre>", start)
        end = len(rest) if end < 0 else end + 6
        out += strip_lines(rest[start:end])
        rest = rest[end:]


def collapse(html):
    out = []
    in_space = False
    for c in html:
        if c.isspace():
            in_space = True
            continue
        if in_space and not (out and out[-1] == ">") and c != "<":
            out.append(" ")
        in_space = False
        out.append(c)
    return "".join(out)


if __name__ == "__main__":
    main()
