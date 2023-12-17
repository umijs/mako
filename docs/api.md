# API

Mako 目前通过 @okamjs/okam 暴露 API 供 node 工具使用，以下为 @okamjs/okam 的 API 说明。

## Usage

比如。

```ts
const { build } = require('@okamjs/okam');
await build({
  root: process.cwd(),
  config: {},
  hooks: {},
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

### hooks

- 类型：

```ts
{
  onCompileLess?: (filePath: string) => Promise<string>;
  onBuildComplete?: (data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      startTime: number;
      endTime: number;
    };
  }) => void;
}
```

- 默认值：`{}`

hooks 是一些钩子函数，用于扩展 Mako 的编译过程。

- `onCompileLess`，用于编译 Less 文件，返回编译后的内容（注：接口近期可能还会有变，因为目前没有支持 SourceMap）
- `onBuildComplete`，在 Build 完成后会调用（注：目前仅在 watch 为 true 时会调用，后续会在 watch 为 false 时也被调用）

### watch

- 类型：`Boolean`
- 默认值：`false`

是否监听文件变化，开启后会启动文件监听服务，当文件变化时会自动重新编译。
