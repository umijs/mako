
## 0.1.3

> @alipay/bigfish@4.1.2

- 修复合并到 common 的 async chunk 在生成 ensure 语句时数据错误导致加载失败的问题 by [@PeachScript](https://github.com/PeachScript) in [#712](https://github.com/umijs/mako/pull/712)
- 修复 require.loadScript 没有替换成 `__mako__require__` 的问题 by [@stormslowly](https://github.com/stormslowly) in [#715](https://github.com/umijs/mako/pull/715)
- 添加 node api 的 dts by [@stormslowly](https://github.com/stormslowly) in [#716](https://github.com/umijs/mako/pull/716)

## 0.1.0

> @alipay/bigfish@4.1.0

- 新增 Emotion 支持 by zhangpanweb in [#694](https://github.com/umijs/mako/pull/694)
- 提升 generate chunks 的性能，m1 yuyanAssets build 稳定提升 200ms 左右 by [@sorrycc](https://github.com/sorrycc) in [#709](https://github.com/umijs/mako/pull/709)
- 提升 generate 阶段 transform 的性能，m1 yuyanAssets 此步骤耗时从 3431ms 降到 1019ms by [@sorrycc](https://github.com/sorrycc) in [#707](https://github.com/umijs/mako/pull/707)
- 修复 window.require 不应该被替换成 `window.__mako_require__` 的问题 by [@jiesia](https://github.com/jiesia) in [#708](https://github.com/umijs/mako/pull/708)
- 修复 transform_in_generate 多线程 transform 时子线程错误没有被处理的问题 by [@sorrycc](https://github.com/sorrycc) in [#710](https://github.com/umijs/mako/pull/710)
- 捕获更多不支持的 loader 语法，比如 file-loader?esModule=false!./src-noconflict/theme-kr_theme.js by [@sorrycc](https://github.com/sorrycc) in [#704](https://github.com/umijs/mako/pull/704)
