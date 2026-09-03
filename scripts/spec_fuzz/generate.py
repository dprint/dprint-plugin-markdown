#!/usr/bin/env python3
"""Writes random markdown documents for `render.js` to render, built out of the
pieces markdown is written with.

    generate.py <seed> <count> <out.json> [block|inline]

`block` (the default) builds documents a line at a time out of a prefix (what
makes a line an item, a quote, a heading) and a body, since that is the shape a
block parser reads. `inline` builds one paragraph at a time out of the pieces
emphasis and links are written with, which is what finds the trouble in how a
decoration's delimiters are chosen.

What the formatter reads as an extension is left out (definition lists, front
matter), along with what commonmark.js reads differently from the spec: a tab
at the end of a line, a tab after a definition's colon, and a non-breaking
space at the edge of a line.
"""
import json
import random
import re
import sys

PREFIXES = ["", "", "", "", " ", "  ", "   ", "    ", "\t", "- ", "* ", "+ ", "1. ", "2) ", "10. ", "> ", ">", "> > ", "# ", "## ", "###### ", "  - ", "    - ", "-", "1.", "[r1]: ", "  [r1]: /u \"t\"", "```", "````", "    ", "=", "==", "---", "***", "<div>", "</div>", "<!-- ", "<b>", "<pre>", "</pre>", "  > ", "- - ", "1. - ", "> - "]
PIECES = [" ", " ", "  ", "\t", "---", "***", "___", "=", "===", "`", "``", "` `", "*", "**", "***", "_", "__", "\\", "\\*", "\\\\", "[", "]", "[r1]", "[r1][]", "[a][r1]", "![a](/u)", "[a](/u)", "[a](/u \"t\")", "[a](<b c>)", "(", ")", "<", ">", "<b>", "</b>", "<b c=\"d\">", "<!-- c -->", "<!---->", "<http://x.y>", "<a@b.c>", "&amp;", "&#35;", "&copy;", "&x;", "#", "\"", "'", "!", "-", "+", "1.", ".", ":", "。", "—", "“", "é", "*a*", "**a**", "_a_", "`a`", "a*", "*a", "a_", "_a", "-a", "a-", "foo", "bar", "baz", "qux", "x", "1", "22", "a1", "ﬁ"]
INLINE_PIECES = ["*", "*", "**", "**", "***", "****", "_", "_", "__", "___", " ", " ", " ", "a", "a", "b", "foo", "1", ".", ",", "!", "'", "\"", "(", ")", "[", "]", "。", "—", "é",
  "\\*", "\\_", "\\\\", "`", "``", "` `", "`a`", "[a](/u)", "[a](/u \"t\")", "[a](<b c>)", "[a](<b\\>c>)", "[a](/u(x))", "[a](/u 'x')", "[a](/u (x))", "[a](<>)", "[a]()", "[a](/u \"x\" )", "[a]( /u )", "[a][r1]", "[r1][]", "[r1]", "[a][r1", "[[a]](/u)", "![a](/u)", "![[a]](/u)", "[![a](/u)](/u2)", "![a][r1]",
  "<a href=\"x\">", "<a  b  = 'c'>", "<a b=c>", "<a/>", "<a >", "<a>", "</a >", "<?x?>", "<!x>", "<![CDATA[x]]>", "<!-- x -->", "<!--->", "<!-->", "<http://x.y>", "<a@b.c>", "&amp;", "&#35;", "&x;", "\n", "  \n", "\\\n"]


def main():
    seed, count, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
    mode = sys.argv[4] if len(sys.argv) > 4 else "block"
    rng = random.Random(seed)
    build = inline_document if mode == "inline" else block_document
    documents = []
    for _ in range(count):
        document = build(rng)
        # front matter is an extension of the formatter, so never open with its fence
        if document.startswith("---") or document.startswith("+++"):
            document = " " + document
        documents.append(document)
    with open(out, "w", encoding="utf-8") as file:
        json.dump(documents, file, ensure_ascii=False)


def block_document(rng):
    lines = [block_line(rng) if rng.random() > 0.15 else "" for _ in range(rng.randint(1, 8))]
    return "\n".join(lines) + ("\n" if rng.random() > 0.2 else "")


def block_line(rng):
    body = "".join(rng.choice(PIECES) for _ in range(rng.randint(0, 6)))
    text = (rng.choice(PREFIXES) + body).rstrip("\t")
    text = text.replace("[r1]: \t", "[r1]: ")
    # a definition list is an extension of the formatter
    if re.match(r"^\s*:(\s|$)", text):
        return block_line(rng)
    return text


def inline_document(rng):
    text = "".join(rng.choice(INLINE_PIECES) for _ in range(rng.randint(1, 14)))
    text = "\n".join(line.rstrip("\t") for line in text.split("\n"))
    if rng.random() < 0.3:
        text += "\n\n[r1]: /u \"t\""
    return text + "\n"


if __name__ == "__main__":
    main()
