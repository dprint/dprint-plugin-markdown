# @dprint/markdown

npm distribution of [dprint-plugin-markdown](https://github.com/dprint/dprint-plugin-markdown).

Use this with [@dprint/formatter](https://github.com/dprint/js-formatter) or just use @dprint/formatter and download the [dprint-plugin-markdown WASM file](https://github.com/dprint/dprint-plugin-markdown/releases).

## Example

```ts
import { createFromBuffer } from "@dprint/formatter";
import { getBuffer } from "@dprint/markdown";

const formatter = createFromBuffer(getBuffer());

console.log(formatter.formatText("test.md", "#   Some Title    "));
```
