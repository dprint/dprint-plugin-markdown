# CommonMark spec data

What `tests/commonmark_spec_check.rs` reads.

- `spec.json` is the examples of the [CommonMark spec](https://spec.commonmark.org/), version 0.31.2, from https://spec.commonmark.org/0.31.2/spec.json. The spec is licensed [CC-BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) by John MacFarlane.
- `entities.json` is the [named character references](https://html.spec.whatwg.org/multipage/named-characters.html) of the HTML standard, from https://html.spec.whatwg.org/entities.json, which the renderer decodes entities with. The HTML standard is licensed [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/) by WHATWG.
- `known_failures.txt` lists the examples that don't come out as the spec says, and why.
