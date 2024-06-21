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

Notice: When you're using mako with Umi, prefer to config the bundler in `.umirc.ts` or `config/config.ts` file.

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

english: If not enabled, only files with `.module.css` or `.module.less` will be treated as CSS Modules; if enabled, named imports like `import styles from './a.css'` will also be treated as CSS Modules.

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
      lib_min_size: 160000
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

Whether to fix flexbugs.

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

- Type: `number`
- Default: `10000`

Specify the size limit of the assets file that needs to be converted to `base64` format.

### less

- Type: `{ modifyVars?: Record<string, string>, sourceMap?: { sourceMapFileInline?: boolean, outputSourceFiles?: boolean }, math?: "always" | "strict" | "parens-division" | "parens" | "strict-legacy" | number, plugins?: ([string, Record<string, any>]|string)[] }`
- Default: `{}`

Specify the less configuration.

e.g.

```ts
{
  modifyVars: {
    'primary-color': '#1DA57A',
    'link-color': '#1DA57A',
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
- Default: mode 为 development 时为 `false`，production 时为 `true`

Whether to minify the code.

### mode

- Type: `"development" | "production"`
- Default: `"development"`

Specify the build mode, `"development"` or `"production"`.

### moduleIdStrategy

- Type: `"named" | "hashed"`
- Default: `"named"` when mode is development, `"hashed"` when mode is production

Specify the strategy for generating moduleId.

### nodePolyfill

- Type: `boolean`
- Default: `true`, and `false` when platform is `node`

Whether to enable node polyfill.

### output

- Type: `{ path: string, mode: "bundle" | "bundless", esVersion: "es3" | "es5" | "es2015" | "es2016" | "es2017" | "es2018" | "es2019" | "es2020" | "es2021" | "es2022" | "esnext", meta: boolean, chunkLoadingGlobal: string, preserveModules: boolean, preserveModulesRoot: string }`
- Default: `{ path: "dist", mode: "bundle", esVersion: "es2022", meta: false, chunkLoadingGlobal: "", preserveModules: false, preserveModulesRoot: "" }`

Output related configuration.

- `path`, output directory
- `mode`, output mode, `"bundle"` or `"bundless"`, default is `"bundle"`
- `esVersion`，output `js` version (Bundless Only)
- `meta`, whether to generate `meta.json` file (Bundless Only)
- `chunkLoadingGlobal`, global variable name for `chunk loading`
- `preserveModules`, whether to preserve the module directory structure (Bundless Only)
- `preserveModulesRoot`, preserve the root directory of the module directory structure (Bundless Only)

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
  buildStart?: () => void;
  generateEnd?: (data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      startTime: number;
      endTime: number;
    };
  }) => void;
  load?: (filePath: string) => Promise<{ content: string, type: 'css'|'js'|'jsx'|'ts'|'tsx' }>;
}
```

JSHooks is a set of hook functions used to extend the compilation process of Mako.

- `name`, plugin name
- `buildStart`, called before Build starts
- `load`, used to load files, return file content and type, type supports `css`, `js`, `jsx`, `ts`, `tsx`
- `generateEnd`, called after Generate completes, `isFirstCompile` can be used to determine if it is the first compilation, `time` is the compilation time, and `stats` is the compilation statistics information

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

publicPath configuration. Note: There is a special value `"runtime"`, which means that it will switch to runtime mode and use the runtime `window.publicPath` as publicPath.

### px2rem

- Type: `false | { root?: number, propBlackList?: string[], propWhiteList?: string[], selectorBlackList?: string[], selectorWhiteList?: string[], minPixelValue?: number }`
- Default: `false`

Whether to enable px2rem conversion.

- `root`, root font size, default is `100`
- `propBlackList`, property black list
- `propWhiteList`, property white list
- `selectorBlackList`, selector black list
- `selectorWhiteList`, selector white list
- `minPixelValue`，minimum pixel value, default is `0`

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

- Type: `false | string`
- Default: `false`

Whether to output umd format.

### useDefineForClassFields

- Type: `boolean`
- Default: `false`

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
