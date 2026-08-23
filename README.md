# dprint-plugin-markdown

[![](https://img.shields.io/crates/v/dprint-plugin-markdown.svg)](https://crates.io/crates/dprint-plugin-markdown) [![CI](https://github.com/dprint/dprint-plugin-markdown/workflows/CI/badge.svg)](https://github.com/dprint/dprint-plugin-markdown/actions?query=workflow%3ACI)

Markdown formatting plugin for dprint.

This uses its own markdown parser, written for formatting rather than rendering: every node knows the exact text it was parsed from and nothing in the file is discarded along the way.

## Install

[Install](https://dprint.dev/install/) and [setup](https://dprint.dev/setup/) dprint.

Then in your project's directory with a dprint.json file, run:

```shellsession
dprint add markdown
```

See https://dprint.dev/plugins/markdown/ for more information.
