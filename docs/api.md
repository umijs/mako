# API

Mako 目前通过 @umijs/mako 暴露 API 供 node 工具使用，以下为 @umijs/mako 的 API 说明。

## Usage

比如。

```ts
const { build } = require('@umijs/mako');
await build({
  root: process.cwd(),
  watch: false,
  config: {},
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

### watch

- 类型：`Boolean`
- 默认值：`false`

是否监听文件变化，开启后会启动文件监听服务，当文件变化时会自动重新编译。
