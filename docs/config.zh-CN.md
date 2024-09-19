# 配置

## 如何配置

在项目的根目录下创建一个 `mako.config.json` 文件，并在其中编写配置。

例如：

```json
{
  "entry": {
    "index": "./src/index.js"
  }
}
```

注意：当你在使用 Mako 与 Umi 时，建议在 `.umirc.ts` 或 `config/config.ts` 文件中配置打包工具。

## 配置项

### analyze

- 类型：`{} | false`
- 默认值：`false`

是否分析构建产物。

注意：此配置项仍在开发中，结果可能不准确。

### autoCSSModules

- 类型：`boolean`
- 默认值：`false`

是否自动启用 CSS Modules。

如果未启用，只有 `.module.css` 或 `.module.less` 的文件会被视为 CSS Modules；如果启用，像 `import styles from './a.css'` 这样的命名导入也会被视为 CSS Modules。

### clean

- 类型：`boolean`
- 默认值：`true`

是否在构建前清理输出目录。

### cjs

- 类型：`boolean`
- 默认值：`false`

是否输出 cjs 格式代码。

### codeSplitting

- 类型：`false | { strategy: "auto" } | { strategy: "granular", options: object } | { strategy: "advanced", options: object }`
- 默认值：`false`

指定代码拆分策略。对于 SPA 使用 `auto` 或 `granular` 策略，对于 MPA 使用 `advance` 策略。

```ts
// auto 策略
{
  codeSplitting: {
    strategy: "auto";
  }
}
```

```ts
// granular 策略
{
  codeSplitting:  {
    strategy: "granular",
    options: {
      // 将被拆分到框架 chunk 的 Node 模块
      frameworkPackages: [ "react", "antd" ],
      // （可选）被拆分的 node 模块的最小大小
      lib_min_size: 160000
    }
  }
}

```

```ts
// advance 策略
{
  codeSplitting: {
    strategy: "advanced",
    options: {
      //（可选）拆分 chunk 的最小大小，小于此大小的异步 chunks 将被合并到入口 chunk
      minSize: 20000,
      // 拆分 chunk 分组配置
      groups: [
        {
          // 分组的名称，当前只支持字符串值
          name: "common",
          //（可选）分组包含模块所属的 chunk 类型，枚举值为 "async"（默认）| "entry" | "all"
          allowChunks: "entry",
          //（可选）分组包含的模块的最小引用次数
          minChunks: 1,
          //（可选）分组生效的最小大小
          minSize: 20000,
          //（可选）分组的最大大小，超过此大小将自动再次拆分
          maxSize: 5000000,
          //（可选）分组的匹配优先级，值越大优先级越高
          priority: 0,
          //（可选）分组的匹配正则表达式
          test: "(?:)",
        }
      ],
    },
  }
}
```

### copy

- 类型：`string[]`
- 默认值：`["public"]`

指定需要复制的文件或目录。默认情况下，会将 `public` 目录下的文件复制到输出目录。

### cssModulesExportOnlyLocales

- 类型：`boolean`
- 默认值：`false`

是否只导出 CSS 模块的类名，而不是 CSS 模块的值。通常用于服务端渲染场景，因为在服务端渲染时，你不需要 CSS 模块的值，只需要类名。

### define

- 类型：`Record<string, string>`
- 默认值：`{ NODE_ENV: "development" | "production }`

指定需要在代码中替换的变量。

例如：

```ts
{
  define: {
    "FOO": "foo",
  },
}
```

注意：当前，define 将自动处理 `process.env` 前缀。

### devServer

- 类型：`false | { host?: string, port?: number }`
- 默认值：`{ host: '127.0.0.1', port: 3000 }`

指定开发服务器的配置。

### devtool

- 类型：`false | "source-map" | "inline-source-map"`
- 默认值：`"source-map"`

指定源映射类型。

### duplicatePackageChecker

- 类型：`{ verbose: boolean, showHelp: boolean, emitError: boolean } | false`
- 默认值：`false`

重复包检查器的配置。

子配置项：

- `verbose`：是否输出详细信息。
- `showHelp`：是否显示帮助信息。
- `emitError`：发现重复包时是否抛出错误。

示例：

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

- 类型：`boolean`
- 默认值：`false`

是否将动态导入转换为 require。在使用 node 平台或希望只有一个 js 输出文件时有用。

例如：

```ts
import("./a.js");
// => require("./a.js")
```

### emitAssets

- 类型：`boolean`
- 默认值：`true`

是否输出资产文件。在构建纯服务端渲染项目时，通常设置为 `false`，因为此时不需要资产文件。

### emitDecoratorMetadata

- Type: `boolean`
- Default: `false`

是否输出 decorator metadata。

### emotion

- 类型：`boolean`
- 默认值：`false`

是否启用 emotion 支持。

### entry

- 类型：`Record<string, string>`
- 默认值：`{}`

指定入口文件。

例如：

```ts
{
  entry: {
    index: "./src/index.js",
    login: "./src/login.js",
  },
}
```

### experimental.detectLoop

- 类型：`false| { "ignoreNodeModules": bool, "graphviz": bool }`
- 默认：`{ "ignoreNodeModules": true, "graphviz": false }`

生成依赖循环信息的实验配置。设置为 `false` 可禁用此功能。

配置项：

- `ignoreNodeModules` 用于忽略包含来自 node_modules 的文件的依赖循环。
- `graphviz` 用于生成名为 `_mako_loop_detector.dot` 的 graphviz dot 文件，用于可视化依赖循环。

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

- 类型：`bool`
- 默认：`true`

实验性配置，用于启用或禁用 [`require.context`](https://webpack.js.org/guides/dependency-management/#requirecontext) 功能。

e.g.

```json
{
  "experimental": {
    "requireContext": false
  }
}
```

### experimental.webpackSyntaxValidate

- 类型：`string[]`
- 默认值：`[]`

实验性配置，指定允许使用 webpack 语法的包。

例如：

```ts
{
  experimental: {
    webpackSyntaxValidate: ["foo", "bar"],
  },
}
```

### externals

- 类型：`Record<string, string>`
- 默认值：`{}`

指定外部依赖的配置。

例如：

```ts
{
  externals: {
    react: "React",
    "react-dom": "ReactDOM",
  },
}
```

那么，当代码遇到 `import React from "react"` 时，它将被替换为 `const React = (typeof globalThis !== 'undefined' ? globalThis : self).React`。

如果你想要以 `require` 的方式输出外部依赖，可以如下设置。

```ts
{
  externals: {
    foo: "commonjs foo",
  },
}
```

那么，当代码遇到 `import foo from "foo"` 时，它将被替换为 `const foo = require("foo")`。

### flexBugs

- 类型：`boolean`
- 默认值：`false`

是否修复 flexbugs。

### forkTsChecker

- 类型：`boolean`
- 默认值：`false`

是否在单独的进程上运行 TypeScript 类型检查器。

### hash

- 类型：`boolean`
- 默认值：`false`

是否生成哈希文件名。

### hmr

- 类型：`false | {}`
- 默认值：`{}`

是否启用热更新。

### ignoreCSSParserErrors

- 类型：`boolean`
- 默认值：`false`

是否忽略 CSS 解析错误。

### ignores

- 类型：`string[]`
- 默认值：`[]`

指定要忽略的文件。被忽略的文件将输出空模块。

例如：

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

- 类型：`{} | false`
- 默认值：`false`

是否将 CSS 内联输出到 JS 中。

注意：此配置仅可与 umd 一起使用，因为注入 CSS 不是推荐的方式，可能会有潜在的性能问题。

### inlineLimit

- 类型：`number`
- 默认值：`10000`

指定需要转换为 `base64` 格式的资产文件的大小限制。


### linlineExcludesRegexes

- 类型: `number`
- 默认值: `10000`

指定不需要转换为 `base64` 格式的资产文件的后缀名列表。

例如：

```ts
{
  "linlineExcludesRegexes": ["webp"]
}
```

### less

- 类型：`{ modifyVars?: Record<string, string>, globalVars?: Record<string, string>, sourceMap?: { sourceMapFileInline?: boolean, outputSourceFiles?: boolean }, math?: "always" | "strict" | "parens-division" | "parens" | "strict-legacy" | number, plugins?: ([string, Record<string, any>]|string)[] }`
- 默认值：`{}`

指定 less 配置。

例如。

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

- 类型：`false | { fileName?: string, basePath?: string }`
- 默认值：`false`

是否生成 `manifest.json` 文件。启用时，默认的 `fileName` 值为 `asset-manifest.json`。

### mdx

- 类型：`boolean`
- 默认值：`false`

是否启用 `mdx` 支持。

### minify

- 类型：`boolean`
- 默认值：mode 为 development 时为 `false`，production 时为 `true`

是否压缩代码。

### mode

- 类型：`"development" | "production"`
- 默认值：`"development"`

指定构建模式，`"development"` 或 `"production"`。

### moduleIdStrategy

- 类型：`"named" | "hashed"`
- 默认值：当 mode 为 development 时为 `"named"`，mode 为 production 时为 `"hashed"`

指定生成 moduleId 的策略。

### nodePolyfill

- 类型：`boolean`
- 默认值：`true`，当平台为 `node` 时为 `false`

是否启用 node polyfill。

### output

- 类型：`{ path: string, mode: "bundle" | "bundless", esVersion: "es3" | "es5" | "es2015" | "es2016" | "es2017" | "es2018" | "es2019" | "es2020" | "es2021" | "es2022" | "esnext", meta: boolean, chunkLoadingGlobal: string, preserveModules: boolean, preserveModulesRoot: string; crossOriginLoading: false | "anonymous" | "use-credentials" }`
- 默认值：`{ path: "dist", mode: "bundle", esVersion: "es2022", meta: false, chunkLoadingGlobal: "", preserveModules: false, preserveModulesRoot: "", crossOriginLoading: false }`

输出相关配置。

- `path`，输出目录
- `mode`，输出模式，`"bundle"` 或 `"bundless"`，默认为 `"bundle"`
- `esVersion`，输出 `js` 版本（仅适用于 Bundless）
- `meta`，是否生成 `meta.json` 文件（仅适用于 Bundless）
- `chunkLoadingGlobal`，`chunk loading` 的全局变量名称
- `preserveModules`，是否保留模块目录结构（仅适用于 Bundless）
- `preserveModulesRoot`，是否保留模块目录结构的根目录（仅限 Bundless）
- `crossOriginLoading`，控制异步 chunk 加载时 `script` 及 `link` 标签的 `crossorigin` 属性值
- `globalModuleRegistry`，是否允许在多 entry 之间共享模块注册中心

### optimization

- 类型：`object`
- 默认值：`{ skipModules: true, concatenateModules: true }`

指定用于优化构建产物的配置。当前支持以下子配置项。

- `skipModules`，通过跳过没有副作用的模块来优化大小
- `concatenateModules`，通过将可以安全合并为一个模块的一组模块在发现的模块树上连接起来，来优化大小

### platform

- 类型：`"browser" | "node"`
- 默认值：`"browser"`

指定构建的平台，`"browser"` 或 `"node"`。

注意：使用 `"node"` 时，还需要将 `dynamicImportToRequire` 设置为 `true`，因为运行时还不支持 node 风格的块加载。

### plugins

- 类型：`(string | JSHooks)[]`
- 默认值：`[]`

指定使用的插件。

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
      ...
    };
  }) => void;
  load?: (filePath: string) => Promise<{ content: string, type: 'css'|'js'|'jsx'|'ts'|'tsx' }>;
}
```

JSHooks 是一组用来扩展 Mako 编译过程的钩子函数。

- `name`，插件名称
- `buildStart`，构建开始前调用
- `load`，用于加载文件，返回文件内容和类型，类型支持 `css`、`js`、`jsx`、`ts`、`tsx`
- `generateEnd`，生成完成后调用，`isFirstCompile` 可用于判断是否为首次编译，`time` 为编译时间，`stats` 是编译统计信息

### progress

- Type: false | { progressChars: string }
- Default: { progressChars: "▨▨" }

是否显示构建进度条。

### providers

- 类型：`Record<string, [string, string]>`
- 默认值：`{}`

指定提供者配置，用于替换代码中的标识符为 require 模块标识符。

例如：

```ts
{
  providers: {
    process: ["process", ""],
    Buffer: ["buffer", "Buffer"],
  },
}
```

这些配置将在遇到时将标识符 `process` 和 `Buffer` 替换为 require 对应模块的代码。

```ts
process;
// => require("process")
Buffer;
// => require("buffer").Buffer
```

### publicPath

- 类型：`string`
- 默认值：`"/"`

publicPath 配置。注意：有一个特殊值 `"runtime"`，这意味着它将切换到运行时模式并使用运行时的 `window.publicPath` 作为 publicPath。

如果你想在运行时设置 `publicPath`，请使用 `__mako_public_path__`。（注：`__webpack_public_path__` 也是支持的）

```ts
__mako_public_path__ = '/foo/';
```

### px2rem

- 类型：`false | { root?: number, propBlackList?: string[], propWhiteList?: string[], selectorBlackList?: string[], selectorWhiteList?: string[], selectorDoubleList?: string[], minPixelValue?: number }`
- 默认值：`false`

是否启用 px2rem 转换。

- `root`，根字体大小，默认为 `100`
- `propBlackList`，属性黑名单
- `propWhiteList`，属性白名单
- `selectorBlackList`，选择器黑名单
- `selectorWhiteList`，选择器白名单
- `selectorDoubleList`，选择器白名单，会被转换为两倍的值
- `minPixelValue`，最小像素值，默认为 `0`
- `mediaQuery`，是否转换媒体查询中的 px, 默认 `false`

其中 `selectorBlackList`、`selectorWhiteList`、`selectorDoubleList` 均支持传递正则表达式或者字符串，如

```json
"selectorBlackList": [".a", "/.__CustomClass_/"]
```

> 被字符 `/` 包裹的字符串会被当作正则表达式解析。

### react

- 类型：`{ runtime: "automatic" | "classic", pragma: string, import_source: string, pragma_frag: string }`
- 默认值：`{ runtime: "automatic", pragma: "React.createElement", import_source: "react", pragma_frag: "React.Fragment" }`

React 相关配置。

例如，

```tsx
function App() {
  return <div>1</div>;
}
```

当运行时为 `automatic` 时，输出如下，

```ts
import { jsx as _jsx } from "react/jsx-runtime";
function App() {
  return /*#__PURE__*/ _jsx("div", {
    children: "1",
  });
}
```

当运行时为 `classic` 时，输出如下，

```ts
function App() {
  return /*#__PURE__*/ React.createElement("div", null, "1");
}
```

### resolve

- 类型：`{ alias: Array<[string, string]>, extensions: string[] }`
- 默认值：`{ alias: [], extensions: ["js", "jsx", "ts", "tsx"] }`

`resolve` 配置。

- `alias`，别名配置
- `extensions`，文件扩展名配置

例如，

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

注意 1：如果你想别名一个目录，请不要添加 `/*` 后缀，我们目前还不支持这样做。

例如，

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

注意 2：如果你想要别名指向一个本地路径，请确保添加 `./` 前缀。否则，它将被视为一个依赖模块。

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

- 类型：`{ logServerComponent: 'error' | 'ignore' } | false`
- 默认值：`false`

与 RSC 客户端相关的配置。

### rscServer

- 类型：`{ clientComponentTpl: string, emitCSS: boolean } | false`
- 默认值：`false`

与 RSC 服务器相关的配置。

子配置项：

- `clientComponentTpl`，客户端组件模板，使用 `{{path}}` 表示组件的路径，使用 `{{id}}` 表示模块的 id。
- `emitCSS`，是否输出 CSS 组件。

### sass

- 类型: `Options<'async'>`
- 默认值: `{}`

> 未安装 `sass` 包。请运行 `npm install sass` 进行安装。

指定 sass [配置](https://sass-lang.com/documentation/js-api/interfaces/options/).


例如：

```ts
{
  "sourceMap": false
}
```

### stats

- 类型：`{ modules: bool } | false`
- 默认值：`false`

是否生成 stats.json 文件。

子配置项：

- `modules`，是否生成模块信息，当你想要分析模块大小但可能会减慢构建速度时，它可能很有用。

### transformImport

- 类型：`false | { libraryName: string, libraryDirectory: string, style: boolean }`
- 默认值：`false`

babel-plugin-import 的简化版本，仅支持三个配置项：libraryName，libraryDirectory 和 style，用于满足现有项目中按需加载 antd v4 样式的需求。

例如：

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

- 类型：`false | string`
- 默认值：`false`

是否输出 umd 格式。

### useDefineForClassFields

- 类型：`boolean`
- 默认值：`true`

是否使用 `defineProperty` 来定义类字段。

### watch

- 类型：`{ ignorePaths: string[] } | false`
- 默认值：`{ ignorePaths: [] }`

与监视相关的配置。

例如，如果你想要忽略根目录下的 `foo` 目录，你可以这样设置。

```ts
{
  watch: {
    ignorePaths: ["foo"],
  },
}
```

### writeToDisk

- 类型：`boolean`
- 默认值：`true`

是否在开发模式下将构建结果写入磁盘。
