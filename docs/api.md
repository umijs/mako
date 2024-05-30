# API

Mako 目前通过 @umijs/mako 暴露 API 供 node 工具使用，以下为 @umijs/mako 的 API 说明。

## Usage

比如。

```ts
const { build } = require('@umijs/mako');
await build({
  root: process.cwd(),
  config: {},
  plugins: [],
  less: {},
  forkTsChecker: true,
  watch: false,
}: BuildOptions);
```

## BuildOptions

### root

- 类型：`String`
- 默认值：`process.cwd()`

项目根目录。

### config

- 类型：`Object`
- 默认值：`{}`

详见[配置](./config.md)。

### less

- 类型：`Object`
- 默认值：`{}`

less 配置。

比如。

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
  ]
}
```

### Plugin

- 类型：

```ts
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

- 默认值：`{}`

hooks 是一些钩子函数，用于扩展 Mako 的编译过程。

- `name`，插件名称
- `buildStart`，在 Build 开始前会调用
- `load`，用于加载文件，返回文件内容和类型，类型支持 `css`、`js`、`jsx`、`ts`、`tsx`
- `generateEnd`，在 Generate 完成后会调用，通过 `isFirstCompile` 可以判断是否是第一次编译，`time` 为编译时间，`stats` 为编译统计信息

### forkTsChecker

- 类型：`boolean`
- 默认值：`false`

是否开启构建时 TypeScript 类型校验。

### watch

- 类型：`Boolean`
- 默认值：`false`

是否监听文件变化，开启后会启动文件监听服务，当文件变化时会自动重新编译。
