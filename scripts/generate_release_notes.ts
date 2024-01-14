import { generateChangeLog } from "https://raw.githubusercontent.com/dprint/automation/0.9.0/changelog.ts";

const version = Deno.args[0];
const changelog = await generateChangeLog({
  versionTo: version,
});
const text = `## Changes

${changelog}

## Install

[Install](https://dprint.dev/install/) and [setup](https://dprint.dev/setup/) dprint.

Then in your project's dprint configuration file:

1. Specify the plugin url in the \`"plugins"\` array or run \`dprint config add markdown\`.
2. Add a \`"markdown"\` configuration property if desired.
   \`\`\`jsonc
   {
     // ...etc...
     "markdown": {
       // markdown config goes here
     },
     "plugins": [
       "https://plugins.dprint.dev/markdown-${version}.wasm"
     ]
   }
   \`\`\`

## JS Formatting API

* [JS Formatter](https://github.com/dprint/js-formatter) - Browser/Deno and Node
* [npm package](https://www.npmjs.com/package/@dprint/markdown)
`;

console.log(text);
