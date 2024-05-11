# Config

> 按字母排序。

## autoCSSModules

- 类型：`boolean`
- 默认值：`false`

是否自动开启 CSS Modules。

如果不开启，只有以 `.module.css` 或 `.module.less` 的文件会被当成 CSS Modules 处理；如果开启，`import styles from './a.css'` 这类具名的 import 也会被当成 CSS Modules 处理。

## clean

- 类型：`boolean`
- 默认值：`true`

是否在 build 前清理输出目录。

## cjs

- 类型：`boolean`
- 默认值：`false`

是否输出 cjs 格式的代码。

## codeSplitting

- 类型：`false | "auto" | object`
- 默认值：`false`

拆包策略，SPA 通常配置为 `auto` 即可，该内置策略会根据项目情况提取 `vendors` chunk 和 `common` chunk；MPA 场景如果需要产出 shared chunk，可以配置为 `object`，配置项说明：

```ts
{
  codeSplitting: {
    //（可选）拆分 chunk 的最小尺寸，单 entry 场景小于该尺寸的 async chunk 会被合并到 entry chunk 中
    minSize: 20000,
    // 拆分 chunk 的分组配置
    groups: [
      {
        // chunk 分组的名称，目前仅支持字符串值
        name: "common",
        //（可选）chunk 分组包含模块所属的 chunk 类型，枚举值为 "async"（默认）| "entry" | "all"
        allowChunks: "entry",
        //（可选）chunk 分组包含模块的最小引用次数
        minChunks: 1,
        //（可选）chunk 分组生效的最小尺寸
        minSize: 20000,
        //（可选）chunk 分组的最大尺寸，超过该尺寸会自动二次拆分
        maxSize: 5000000,
        //（可选）chunk 分组的匹配优先级，值越大优先级越高
        priority: 0,
        //（可选）chunk 分组的匹配正则
        test: "(?:)",
      }
    ],
  },
}
```

## copy

- 类型：`string[]`
- 默认值：`["public"]`

需要拷贝的文件或目录，默认会拷贝 `public` 目录下的文件到输出目录。

## cssModulesExportOnlyLocales

- 类型：`boolean`
- 默认值：`false`

是否只导出 CSS Modules 的类名，而不导出 CSS Modules 的值。通常用于服务端渲染场景，因为服务端渲染时不需要 CSS Modules 的值，只需要类名即可。

## define

- 类型：`Record<string, string>`
- 默认值：`{ NODE_ENV: "development" | "production }`

定义的全局变量。

比如。

```ts
{
  define: {
    "FOO": "foo",
  },
}
```

注：目前的 define 会自动处理 `process.env` 前缀。

## devtool

- 类型：`false | "source-map" | "inline-source-map"`
- 默认值：`"source-map"`

Source Map 类型。

## dynamicImportToRequire

- 类型：`boolean`
- 默认值：`false`

是否将动态 import 转换成 require。

配置后，比如。

```ts
import("./a.js")
// => require("./a.js")
```

## emitAssets

- 类型：`boolean`
- 默认值：`true`

是否输出 assets 文件。通常用于构建纯服务端渲染的项目时设置为 `false`，因为此时不需要输出 assets 文件。

## emotion

- 类型：`boolean`
- 默认值：`false`

是否开启 emotion 支持。

## entry

- 类型：`Record<string, string>`
- 默认值：`{}`

入口文件的配置。比如。

```ts
{
  entry: {
    index: "./src/index.js",
    login: "./src/login.js",
  },
}
```

## experimental.webpackSyntaxValidate

- 类型：`string[]`
- 默认值: `[]`

实验性配置，指定允许使用 webpack 语法的包。比如。

```ts
{
  experimental: {
    webpackSyntaxValidate: ["foo", "bar"],
  },
}
```

## externals

- 类型：`Record<string, string>`
- 默认值：`{}`

外部依赖的配置。比如。

```ts
{
  externals: {
    react: "React",
    "react-dom": "ReactDOM",
  },
}
```

注：external 配置的值还有高级的配置模式，通常用不到，所以这里不展开，有需要可查看源码。

## flexBugs

- 类型：`boolean`
- 默认值：`false`

是否修复 flexbugs。

## hash

- 类型：`boolean`
- 默认值：`false`

是否生成 hash 文件名。

注：后续会改成 `object` 格式，以方便用户做更多控制。

## hmr

- 类型：`false | { host?: string, port?: number }`
- 默认值：`{ host: '127.0.0.1', port: 3000 }`

是否开启热更新。

## ignoreCSSParserErrors

- 类型：`boolean`
- 默认值：`false`

是否忽略 CSS 解析错误。

默认配置是，项目 CSS 有错误就报错，node_modules 下的 CSS 不报错，因为 node_modules 下的 CSS 有很多是不符合规范的，但却不是自己可控的。`ignoreCSSParserErrors` 配置的作用是忽略项目下的 CSS 报错，比如用 Less 编译 less 文件时，会把 node_modules 下的 less 文件也编译进去，此时不能区分是否来自 node_modules 下，而用户对来自 node_modules 下的代码又没有控制权，所以加此配置项。

## ignores

- 类型：`string[]`
- 默认值：`[]`

需要忽略的文件。忽略的文件会输出空模块。

比如。

```ts
{
  "ignores": [
    "^assert$",
    "xxxx.provider.js$",
    "^(node:)?({})(/|$)"
  ]
}
```

## inlineCSS

- 类型：`{} | false`
- 默认值：`false`

是否以内联到 JS 的方式输出 CSS。

注：此配置只能在 umd 开始时使用，因为注入 CSS 并不是推荐的方式，可能有潜在的性能问题。

## inlineLimit

- 类型：`number`
- 默认值：`10000`

小于 `inlineLimit` 大小的 assets 文件会被转换成 `base64` 格式。

## manifest

- 类型：`false | { fileName?: string, basePath?: string }`
- 默认值：`false`

是否生成 `manifest.json` 文件，启用时 `fileName` 的默认值为 `asset-manifest.json`。

## mdx

- 类型：`boolean`
- 默认值：`false`

是否开启 `mdx` 支持。

## minify

- 类型：`boolean`
- 默认值：mode 为 development 时为 `false`，production 时为 `true`

是否压缩代码。

注：后续会改成 `Object` 类型，支持更多子配置用于控制压缩参数。

## mode

- 类型：`"development" | "production"`
- 默认值：`"development"`

构建模式，`"development"` 或 `"production"`。

## moduleIdStrategy

- 类型：`"named" | "hashed"`
- 默认值：mode 为 development 时是 `"named"`，production 时是 `"hashed"`

moduleId 的生成策略。

## nodePolyfill

- 类型：`boolean`
- 默认值：`true`，但 platform 为 `"node"` 时为 `false`

是否开启 node polyfill。

## output

- 类型：`{ path: string, mode: "bundle" | "bundless", esVersion: "es3" | "es5" | "es2015" | "es2016" | "es2017" | "es2018" | "es2019" | "es2020" | "es2021" | "es2022" | "esnext", meta: boolean, chunkLoadingGlobal: string, preserveModules: boolean, preserveModulesRoot: string }`
- 默认值：`{ path: "dist", mode: "bundle", esVersion: "es2022", meta: false, chunkLoadingGlobal: "", preserveModules: false, preserveModulesRoot: "" }`

和输出相关的配置。

- `path`，输出目录
- `mode`，输出模式，`"bundle"` 或 `"bundless"`，默认为 `"bundle"`
- `esVersion`，输出的 `js` 版本（注：Bundless Only）
- `meta`，是否生成 `meta.json` 文件（注：Bundless Only）
- `chunkLoadingGlobal`，`chunk loading` 的全局变量名
- `preserveModules`，是否保留模块目录结构（注：Bundless Only）
- `preserveModulesRoot`，保留模块目录结构的根目录（注：Bundless Only）

## optimization

- 类型：`object`
- 默认值：`{ skipModules: true, concatenateModules: true }`

优化构建产物的配置。目前支持子配置项如下。

- `skipModules`，通过跳过无副作用的模块，优化尺寸
- `concatenateModules`，通过查找的模块树上可以安全合并的一组模块，将它们合并成为一个模块的方式来优化产物尺寸

## optimizePackageImports

- 类型：`boolean`
- 默认值：`false`

是否优化 package imports。

注：实验属性，暂时勿用。

## platform

- 类型：`"browser" | "node"`
- 默认值：`"browser"`

构建平台，`"browser"` 或 `"node"`。

注：使用 `"node"` 时，目前还需设置 `dynamicImportToRequire` 为 `true`，因为 runtime 还不支持 node 方式的 chunk 加载。

## providers

- 类型：`Record<string, [string, string]>`
- 默认值：`{}`

提供者配置，用于替换代码中的标识符为 require 模块的方式。

比如。

```ts
{
  providers: {
    process: ["process", ""],
    Buffer: ["buffer", "Buffer"],
  },
}
```

以上配置会在遇到 `process` 和 `Buffer` 标识符时将其替换为 require 对应模块的代码。

```ts
process
// => require("process")
Buffer
// => require("buffer").Buffer
```

## publicPath

- 类型：`string`
- 默认值：`"/"`

publicPath 配置。注：有个特殊值 `"runtime"`，表示会切换到 runtime 模式，使用运行时的 `window.publicPath` 作为 publicPath。

## px2rem

- 类型：`false | { root?: number, propBlackList?: string[], propWhiteList?: string[], selectorBlackList?: string[], selectorWhiteList?: string[], minPixelValue?: number }`
- 默认值：`false`

是否开启 px2rem 转换。

- `root`，根节点的字体大小，默认值为 `100`
- `propBlackList`，属性黑名单
- `propWhiteList`，属性白名单
- `selectorBlackList`，选择器黑名单
- `selectorWhiteList`，选择器白名单
- `minPixelValue`，最小像素值，默认值为 `0`

## react

- 类型：`{ runtime: "automatic" | "classic", pragma: string, import_source: string, pragma_frag: string }`
- 默认值：`{ runtime: "automatic", pragma: "React.createElement", import_source: "react", pragma_frag: "React.Fragment" }`

react 编译相关配置。

比如。

```tsx
function App() {
  return <div>1</div>;
}
```

runtime 为 automatic 时的产物如下，

```ts
import { jsx as _jsx } from "react/jsx-runtime";
function App() {
  return /*#__PURE__*/_jsx("div", {
    children: "1"
  });
}
```

runtime 为 classic 时的产物如下，

```ts
function App() {
  return /*#__PURE__*/React.createElement("div", null, "1");
}
```

## resolve

- 类型：`{ alias: Record<string, string>, extensions: string[] }`
- 默认值：`{ alias: {}, extensions: ["js", "jsx", "ts", "tsx"] }`

`resolve` 相关配置。

- `alias`，别名配置
- `extensions`，文件扩展名配置

比如：

```ts
{
  resolve: {
    alias: {
      "@": "./src",
    },
    extensions: ["js", "jsx", "ts", "tsx"],
  },
}
```

## stats

- 类型：`boolean`
- 默认值：`false`

是否生成 stats.json 文件。

## transformImport

- 类型：`false | { libraryName: string, libraryDirectory: string, style: boolean }`
- 默认值：`false`

简化版 babel-plugin-import，仅支持 libraryName、libraryDirectory 及 style 三个配置项，用于满足存量项目 antd v4 样式按需加载的需求。

比如：

```ts
{
  transformImport: {
    libraryName: "antd",
    libraryDirectory: "es",
    style: true,
  },
}
```

## umd

- 类型：`false | string`
- 默认值：`false`

是否输出 umd 格式的代码。

注：后续会改成 `Object` 类型，支持更多子配置用于控制 umd 参数。

## writeToDisk

- 类型：`boolean`
- 默认值：`true`

是否在 mode 为 development 时将构建结果写入磁盘。
