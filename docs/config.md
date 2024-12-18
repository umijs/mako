# Config

## How to config

Create a `mako.config.json` file in the root directory of your project, and write the configuration in it.

e.g.

```json
{
  "entry": {
    "index": "./src/index.js"
  }
}
```

Notice: When you're using Mako with Umi, prefer to config the bundler in `.umirc.ts` or `config/config.ts` file.

## Configuration items

### analyze

- Type: `{} | false`
- Default: `false`

Whether to analyze the build artifacts.

Notice: this configuration item is still WIP, the result may not be accurate.

### autoCSSModules

- Type: `boolean`
- Default: `false`

Whether to automatically enable CSS Modules.

If not enabled, only files with `.module.css` or `.module.less` will be treated as CSS Modules; if enabled, named imports like `import styles from './a.css'` will also be treated as CSS Modules.

### caseSensitiveCheck

- Type: `boolean`
- Default: `true`

Whether to enable case-sensitive check.

e.g.

```ts
{
  caseSensitiveCheck: false,
}
```


### clean

- Type: `boolean`
- Default: `true`

Whether to clean the output directory before building.

### cjs

- Type: `boolean`
- Default: `false`

Whether to output cjs format code.

### codeSplitting

- Type: `false |  { strategy: "auto" } | { strategy: "granular", options: object } | { strategy: "advanced", options: object }`
- Default: `false`

Specify the code splitting strategy. Use `auto` or `granular` strategy for SPA, and `advance` strategy for MPA.

```ts
// auto strategy
{
  codeSplitting: {
    strategy: "auto";
  }
}
```

```ts
// granular strategy
{
  codeSplitting:  {
    strategy: "granular",
    options: {
      // Node modules those will be split to framework chunk
      frameworkPackages: [ "react", "antd" ],
      // (optional) The minimum size of the node module to be split
      libMinSize: 160000
    }
  }
}

```

```ts
// advance strategy
{
  codeSplitting: {
    strategy: "advanced",
    options: {
      //（optional）The minimum size of the split chunk, async chunks smaller than this size will be merged into the entry chunk
      minSize: 20000,
      // Split chunk grouping configuration
      groups: [
        {
          // The name of the chunk group, currently only string values are supported
          name: "common",
          //（optional）The chunk type that the chunk group contains modules belong to, enum values are "async" (default) | "entry" | "all"
          allowChunks: "entry",
          //（optional）The minimum number of references to modules contained in the chunk group
          minChunks: 1,
          //（optional）The minimum size of the chunk group to take effect
          minSize: 20000,
          //（optional）The maximum size of the chunk group, exceeding this size will be automatically split again
          maxSize: 5000000,
          //（optional）The matching priority of the chunk group, the larger the value, the higher the priority
          priority: 0,
          //（optional）The matching regular expression of the chunk group
          test: "(?:)",
        }
      ],
    },
  }
}
```

### copy

- Type: `string[]`
- Default: `["public"]`

Specify the files or directories to be copied. By default, the files under the `public` directory will be copied to the output directory.

### cssModulesExportOnlyLocales

- Type: `boolean`
- Default: `false`

Whether to export only the class names of CSS Modules, not the values of CSS Modules. Usually used in server-side rendering scenarios, because when server-side rendering, you don't need the values of CSS Modules, only the class names are needed.

### define

- Type: `Record<string, string>`
- Default: `{ NODE_ENV: "development" | "production }`

Specify the variables that need to be replaced in the code.

e.g.

```ts
{
  define: {
    "FOO": "foo",
  },
}
```

Notice: Currently, define will automatically handle the `process.env` prefix.

### devServer

- Type: `false | { host?: string, port?: number }`
- Default: `{ host: '127.0.0.1', port: 3000 }`

Specify the devServer configuration.

### devtool

- Type: `false | "source-map" | "inline-source-map"`
- Default: `"source-map"`

Specify the source map type.

### duplicatePackageChecker

- Type: `{ verbose: boolean, showHelp: boolean, emitError: boolean } | false`
- Default: `false`

Configuration for duplicate package checker.

Child configuration items:

- `verbose`: Whether to output detailed information.
- `showHelp`: Whether to show help information.
- `emitError`: Whether to emit an error when duplicate packages are found.

Example:

```json
{
  "duplicatePackageChecker": {
    "verbose": true,
    "showHelp": true,
    "emitError": false
  }
}
```

### dynamicImportToRequire

- Type: `boolean`
- Default: `false`

Whether to convert dynamic import to require. Useful when using node platform, or when you want just a single js output file.

e.g.

```ts
import("./a.js");
// => require("./a.js")
```

### emitAssets

- Type: `boolean`
- Default: `true`

Whether to output assets files. Usually set to `false` when building a pure server-side rendering project, because assets files are not needed at this time.

### emitDecoratorMetadata

- Type: `boolean`
- Default: `false`

Whether to emit decorator metadata.

### emotion

- Type: `boolean`
- Default: `false`

Whether to enable emotion support.

### entry

- Type: `Record<string, string>`
- Default: `{}`

Specify the entry file.

e.g.

```ts
{
  entry: {
    index: "./src/index.js",
    login: "./src/login.js",
  },
}
```
### experimental.detectLoop

- Type: `false| { "ignoreNodeModules": bool, "graphviz": bool }`
- Default: `{ "ignoreNodeModules": true, "graphviz": false }`

Experimental configuration for generating dependence loop info. `false` to disable the feature.

Options:

- `ignoreNodeModules` to ignore dependence loops which contains files from  node_modules.
- `graphviz` to generate a graphviz dot file named `_mako_loop_detector.dot` at root of project for visualizing dependence loops.

e.g.

```json
{
  "experimental": {
    "ignoreNodeModules": false,
    "graphviz": true
  }
}
```

### experimental.requireContext

- Type: `bool`
- Default: `true`

Experimental configuration, to enable or disable the [`require.context`](https://webpack.js.org/guides/dependency-management/#requirecontext) feature.

e.g.

```json
{
  "experimental": {
    "requireContext": false
  }
}
```

### experimental.webpackSyntaxValidate

- Type: `string[]`
- Default: `[]`

Experimental configuration, specify the packages that are allowed to use webpack syntax.

e.g.

```ts
{
  experimental: {
    webpackSyntaxValidate: ["foo", "bar"],
  },
}
```

### experimental.magicComment

- Type: boolean
- Default: false

Experimental configuration, whether to support magic comments like webpack.

e.g.

```ts
{
  experimental: {
    magicComment: true,
  },
}
```

the magic comment is like below:

```ts
import(/* makoChunkName: 'myChunk' */ "./lazy");
import(/* webpackChunkName: 'myChunk' */ "./lazy");
new Worker(/* makoChunkName: 'myWorker' */ new URL("./worker", import.meta.url));
new Worker(/* webpackChunkName: 'myWorker' */ new URL("./worker", import.meta.url));
import(/* makoIgnore: true */ "./foo");
import(/* webpackIgnore: true */ "./foo");
```

### externals

- Type: `Record<string, string>`
- Default: `{}`

Specify the configuration of external dependencies.

e.g.

```ts
{
  externals: {
    react: "React",
    "react-dom": "ReactDOM",
  },
}
```

Then, when the code encounters `import React from "react"`, it will be replaced with `const React = (typeof globalThis !== 'undefined' ? globalThis : self).React`.

If you want to output the external dependencies with `require`, you can set it as follows.

```ts
{
  externals: {
    foo: "commonjs foo",
  },
}
```

Then, when the code encounters `import foo from "foo"`, it will be replaced with `const foo = require("foo")`.

### flexBugs

- Type: `boolean`
- Default: `false`

Whether to fix flexBugs.

### forkTsChecker

- Type: `boolean`
- Default: `false`

Whether to run TypeScript type checker on a separate process.

### hash

- Type: `boolean`
- Default: `false`

Whether to generate hash file names.

### hmr

- Type: `false | {}`
- Default: `{}`

Whether to enable hot update.

### ignoreCSSParserErrors

- Type: `boolean`
- Default: `false`

Whether to ignore CSS parsing errors.

### ignores

- Type: `string[]`
- Default: `[]`

Specifies the files to be ignored. Ignored files will output empty modules.

e.g.

```ts
{
  "ignores": [
    "^assert$",
    "xxxx.provider.js$",
    "^(node:)?({})(/|$)"
  ]
}
```

### inlineCSS

- Type: `{} | false`
- Default: `false`

Whether to output CSS inlined into JS.

Notice: This configuration can only be used with umd, because injecting CSS is not a recommended way and may have potential performance issues.

### inlineLimit

- Type: `string[]`
- Default: `[]`

Specify the size limit of the assets file that needs to be converted to `base64` format.


### inlineExcludesExtensions

- Type: `string[]`
- Default: `[]`

Excludes assets file extension list that don't need to be converted to `base64` format.

e.g.

```ts
{
  "inlineExcludesExtensions": ["webp"]
}
```


### less

- Type: `{ modifyVars?: Record<string, string>, globalVars?: Record<string, string>, sourceMap?: { sourceMapFileInline?: boolean, outputSourceFiles?: boolean }, math?: "always" | "strict" | "parens-division" | "parens" | "strict-legacy" | number, plugins?: ([string, Record<string, any>]|string)[] }`
- Default: `{}`

Specify the less configuration.

e.g.

```ts
{
  modifyVars: {
    'primary-color': '#1DA57A',
    'link-color': '#1DA57A',
  },
  globalVars: {
    'primary-color': '#ffff00',
    hack: 'true; @import "your-global-less-file.less";',
  },
  sourceMap: {
    sourceMapFileInline: true,
    outputSourceFiles: true,
  },
  math: 'always',
  plugins: [
    [require.resolve("less-plugin-clean-css"), { roundingPrecision: 1 }]
  ],
}
```

### manifest

- Type: `false | { fileName?: string, basePath?: string }`
- Default: `false`

Whether to generate the `manifest.json` file. When enabled, the default value of `fileName` is `asset-manifest.json`.

### mdx

- Type: `boolean`
- Default: `false`

Whether to enable `mdx` support.

### minify

- Type: `boolean`
- Default: mode will be `false` when mode is development, and `true` when mode is production

Whether to minify the code.

### mode

- Type: `"development" | "production"`
- Default: `"development"`

Specify the build mode, `"development"` or `"production"`.

### moduleIdStrategy

- Type: `"named" | "hashed" | "numeric"`
- Default: `"named"` when mode is development, `"hashed"` when mode is production

Specify the strategy for generating moduleId.

### nodePolyfill

- Type: `boolean`
- Default: `true`, and `false` when platform is `node`

Whether to enable node polyfill.

### output

- Type: `{ path: string, mode: "bundle" | "bundless", esVersion: "es3" | "es5" | "es2015" | "es2016" | "es2017" | "es2018" | "es2019" | "es2020" | "es2021" | "es2022" | "esnext", meta: boolean, chunkLoadingGlobal: string, preserveModules: boolean, preserveModulesRoot: string; crossOriginLoading: false | "anonymous" | "use-credentials" }`
- Default: `{ path: "dist", mode: "bundle", esVersion: "es2022", meta: false, chunkLoadingGlobal: "", preserveModules: false, preserveModulesRoot: "", crossOriginLoading: false }`

Output related configuration.

- `path`, output directory
- `mode`, output mode, `"bundle"` or `"bundless"`, default is `"bundle"`
- `esVersion`，output `js` version (Bundless Only)
- `meta`, whether to generate `meta.json` file (Bundless Only)
- `chunkLoadingGlobal`, global variable name for `chunk loading`
- `preserveModules`, whether to preserve the module directory structure (Bundless Only)
- `preserveModulesRoot`, preserve the root directory of the module directory structure (Bundless Only)
- `crossOriginLoading`, control the `crossorigin` attribute of the `script` tag and `link` tag for load async chunks
- `globalModuleRegistry`, whether enable shared module registry across multi entries

### optimization

- Type: `object`
- Default: `{ skipModules: true, concatenateModules: true }`

Specify the configuration to optimize the build artifacts. Currently, the following sub-configuration items are supported.

- `skipModules`, optimize the size by skipping modules without side effects
- `concatenateModules`, optimize the size by concatenating a group of modules that can be safely merged on the found module tree into one module

### platform

- Type: `"browser" | "node"`
- Default: `"browser"`

Specify the platform to build, `"browser"` or `"node"`.

Notice: When using `"node"`, you also need to set `dynamicImportToRequire` to `true`, because the runtime does not yet support node-style chunk loading.

### plugins

- Type: `(string | JSHooks)[]`
- Default: `[]`

Specify the plugins to use.

```ts
// JSHooks
{
  name?: string;
  enforce?: "pre" | "post";
  buildStart?: () => void;
  buildEnd?: () => void;
  generateEnd?: (data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      startTime: number;
      endTime: number;
      ...
    };
  }) => void;
  writeBundle?: () => void;
  watchChanges?: (id: string, params: { event: "create" | "delete" | "update" }) => void;
  load?: (filePath: string) => Promise<{ content: string, type: 'css'|'js'|'jsx'|'ts'|'tsx' }>;
  loadInclude?: (filePath: string) => boolean;
  resolveId?: (id: string, importer: string, { isEntry: bool }) => Promise<{ id: string, external: bool }>;
  transform?: (content: string, id: string) => Promise<{ content: string, type: 'css'|'js'|'jsx'|'ts'|'tsx' }>;
  transformInclude?: (filePath: string) => Promise<boolean> | boolean;
}
```

JSHooks is a set of hook functions used to extend the compilation process of Mako.

- `name`, plugin name
- `buildStart`, called before Build starts
- `load`, used to load files, return file content and type, type supports `css`, `js`, `jsx`, `ts`, `tsx`
- `generateEnd`, called after Generate completes, `isFirstCompile` can be used to determine if it is the first compilation, `time` is the compilation time, and `stats` is the compilation statistics information

### progress

- Type: false | { progressChars: string }
- Default: { progressChars: "▨▨" }

Whether to display the build progress bar.

### providers

- Type: `Record<string, [string, string]>`
- Default: `{}`

Specify the provider configuration, used to replace identifiers in the code with require module identifiers.

e.g.

```ts
{
  providers: {
    process: ["process", ""],
    Buffer: ["buffer", "Buffer"],
  },
}
```

These configurations will replace the identifiers `process` and `Buffer` with the code that require the corresponding module when encountered.

```ts
process;
// => require("process")
Buffer;
// => require("buffer").Buffer
```

### publicPath

- Type: `string`
- Default: `"/"`

publicPath configuration. Note: There is two special values

- `"runtime"`, which means that it will switch to runtime mode and use the runtime `window.publicPath` as publicPath.

- `"auto"`, which is just like `publicPath: "auto"` in webpack

If you want to set the `publicPath` in the runtime, use `__mako_public_path__`. (Notice: `__webpack_public_path__` is also supported)

```ts
__mako_public_path__ = '/foo/';
```

### px2rem

- Type: `false | { root?: number, propBlackList?: string[], propWhiteList?: string[], selectorBlackList?: string[],
  selectorWhiteList?: string[], selectorDoubleList?: string[], minPixelValue?: number, mediaQuery?: boolean }`
- Default: `false`

Whether to enable px2rem conversion.

- `root`, root font size, default is `100`
- `propBlackList`, property black list
- `propWhiteList`, property white list
- `selectorBlackList`, selector black list
- `selectorWhiteList`, selector white list
- `selectorDoubleList`, selector double rem list
- `minPixelValue`，minimum pixel value, default is `0`
- `mediaQuery`，allow px to be converted in media queries, default is `false`

Among them, `selectorBlackList`, `selectorWhiteList` and `selectorDoubleList` all support passing regular expressions or strings, such as

```json
"selectorBlackList": [".a", "/.__CustomClass_/"]
```

> The string wrapped by the characters `/` will be parsed as a regular expression.

### react

- Type: `{ runtime: "automatic" | "classic", pragma: string, import_source: string, pragma_frag: string }`
- Default: `{ runtime: "automatic", pragma: "React.createElement", import_source: "react", pragma_frag: "React.Fragment" }`

react related configuration.

e.g.

```tsx
function App() {
  return <div>1</div>;
}
```

When runtime is `automatic`, the output is as follows,

```ts
import { jsx as _jsx } from "react/jsx-runtime";
function App() {
  return /*#__PURE__*/ _jsx("div", {
    children: "1",
  });
}
```

When runtime is `classic`, the output is as follows,

```ts
function App() {
  return /*#__PURE__*/ React.createElement("div", null, "1");
}
```

### resolve

- Type: `{ alias: Array<[string, string]>, extensions: string[] }`
- Default: `{ alias: [], extensions: ["js", "jsx", "ts", "tsx"] }`

`resolve` configuration.

- `alias`, alias configuration
- `extensions`, file extensions configuration

e.g.

```ts
{
  resolve: {
    alias: [
      ["@", "./src"]
    ],
    extensions: ["js", "jsx", "ts", "tsx"],
  },
}
```

Notice 1: If you want to alias a directory, please don't add the `/*` affix, we don't support it yet.

e.g.

```diff
{
  resolve: {
    alias: [
-      ["@/src/*", "./src/*"],
+      [ "@/src", "./src"],
    ],
  },
}
```

Notice 2: If you want to alias to a local path, make sure to add the `./` prefix. Otherwise, it will be treated as a dependency module.

```diff
{
  resolve: {
    alias: [
-       ["@/src", "src"],
+       ["@/src", "./src"],
    ],
  },
}
```

### rscClient

- Type: `{ logServerComponent: 'error' | 'ignore' } | false`
- Default: `false`

Configuration related to RSC client.

### rscServer

- Type: `{ clientComponentTpl: string, emitCSS: boolean } | false`
- Default: `false`

Configuration related to RSC server.

Child configuration items:

- `clientComponentTpl`, client component template, use `{{path}}` to represent the path of the component, and use `{{id}}` to represent the id of the module.
- `emitCSS`, whether to output CSS components.

### sass

- Type: `Options<'async'>`
- Default: `{}`

> The "sass" package is not installed. Please run "npm install sass" to install it.

Specify the sass [configuration](https://sass-lang.com/documentation/js-api/interfaces/options/).


e.g.

```ts
{
  "sourceMap": false
}
```

### stats

- Type: `{ modules: bool } | false`
- Default: `false`

Whether to generate stats.json file.

Child configuration items:

- `modules`, whether to generate module information, it may be useful when you want to analyze the size of the module but may slow down the build speed.

### transformImport

- Type: `false | { libraryName: string, libraryDirectory: string, style: boolean }`
- Default: `false`

Simplified version of babel-plugin-import, only supports three configuration items: libraryName, libraryDirectory, and style, used to meet the needs of on-demand loading of antd v4 styles in stock projects.

e.g.

```ts
{
  transformImport: {
    libraryName: "antd",
    libraryDirectory: "es",
    style: true,
  },
}
```

### umd

- Type: `false | string | { name: string, export?: string [] }`
- Default: `false`

Whether to output umd format.

### useDefineForClassFields

- Type: `boolean`
- Default: `true`

Whether to use `defineProperty` to define class fields.

### watch

- Type: `{ ignorePaths: string[] } | false`
- Default: `{ ignorePaths: [] }`

Watch related configuration.

e.g. If you want to ignore the `foo` directory under root directory, you can set it as follows.

```ts
{
  watch: {
    ignorePaths: ["foo"],
  },
}
```

### writeToDisk

- Type: `boolean`
- Default: `true`

Whether to write the build result to disk when mode is development.

