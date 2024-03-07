# dprint-plugin-markdown

[![](https://img.shields.io/crates/v/dprint-plugin-markdown.svg)](https://crates.io/crates/dprint-plugin-markdown) [![CI](https://github.com/dprint/dprint-plugin-markdown/workflows/CI/badge.svg)](https://github.com/dprint/dprint-plugin-markdown/actions?query=workflow%3ACI)

Markdown formatting plugin for dprint.

This uses the [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) parser for markdown.

## Building `.wasm` File as Plugin for `dprint`

If you want to build a `.wasm` file to be used as a plugin for `dprint` the following command can be used:

```
cargo build --release --target=wasm32-unknown-unknown --features="wasm"
```

The `--release` parameter is optional. The generated `.wasm` file can then be found in the target directory.

Note that this requires adding `wasm32` as a Rust target. This can be done using the following command:

```
rustup target add wasm32-unknown-unknown
```
