
## 0.5.3

`2024-06-04`

- 修复: 更新 chunk URL 映射，当在监视模式中添加异步导入时 by [@xusd320](https://github.com/xusd320) in [#1220](https://github.com/umijs/mako/pull/1220)
- 修复: 非点号开头的模式未匹配 by [@stormslowly](https://github.com/stormslowly) in [#1230](https://github.com/umijs/mako/pull/1230)
- 修复(fix_helper_inject_position): 导出变量 ctxt 缺失 by [@sorrycc](https://github.com/sorrycc) in [#1236](https://github.com/umijs/mako/pull/1236)
- 优化: 更新 mako bundler 以适应新的 mako 版本 by [@Jinbao1001](https://github.com/Jinbao1001) in [#1229](https://github.com/umijs/mako/pull/1229)

## 0.5.2

`2024-05-31`

- 新增(experimental): SSU 提供功能 by [@stormslowly](https://github.com/stormslowly) in [#1186](https://github.com/umijs/mako/pull/1186)
- 修复: 当 hmr 为 false 时不生成 hmr chunk 和 json by [@sorrycc](https://github.com/sorrycc) in [#1223](https://github.com/umijs/mako/pull/1223)
- 修复: chunk 运行时模板无法兼容旧设备 by [@PeachScript](https://github.com/PeachScript) in [#1227](https://github.com/umijs/mako/pull/1227)
- 其他：支持本地使用 musl 发布 by [@sorrycc](https://github.com/sorrycc) in [#1221](https://github.com/umijs/mako/pull/1221)

## 0.5.1

`2024-05-30`

- 新增插件形式扩展 mako 功能 by [@sorrycc](https://github.com/sorrycc) in [#1219](https://github.com/umijs/mako/pull/1219)
- 新增 x86\_64 linux musl 支持 by [@stormslowly](https://github.com/stormslowly) in [#1218](https://github.com/umijs/mako/pull/1218)
- 修复模块合并获取模块导出符号解析的错误 by [@stormslowly](https://github.com/stormslowly) in [#1216](https://github.com/umijs/mako/pull/1216)
- 修复循环依赖下导致 HRM 报错的问题 by [@stormslowly](https://github.com/stormslowly) in [#1191](https://github.com/umijs/mako/pull/1191)

## 0.5.0

`2024-05-29`

* 新增 watch.ignorePaths 配置 by [@sorrycc](https://github.com/sorrycc) in [#1179](https://github.com/umijs/mako/pull/1179)
* 新增支持 externals 和 commonjs require by [@sorrycc](https://github.com/sorrycc) in [#1185](https://github.com/umijs/mako/pull/1185)
* 新增 rscClient.logServerComponent 配置 by [@sorrycc](https://github.com/sorrycc) in [#1200](https://github.com/umijs/mako/pull/1200)
* 新增 stats.modules 配置以生成具有依赖项和依赖项的模块 by [@sorrycc](https://github.com/sorrycc) in [#1202](https://github.com/umijs/mako/pull/1202)
* 新增 useDefineForClassFields 配置 by [@stormslowly](https://github.com/stormslowly) in [#1181](https://github.com/umijs/mako/pull/1181)
* 优化 watch、dev_server 和 hmr 配置（含 Break Change） by [@sorrycc](https://github.com/sorrycc) in [#1206](https://github.com/umijs/mako/pull/1206)
* 优化改进 parseServerStats by [@sorrycc](https://github.com/sorrycc) in [#1203](https://github.com/umijs/mako/pull/1203)
* 修复 hooks 丢失传输问题 by [@Jinbao1001](https://github.com/Jinbao1001) in [#1170](https://github.com/umijs/mako/pull/1170)
* 修复 with-antd 示例在 watch 时 “too many files open” 错误 by [@zhangpanweb](https://github.com/zhangpanweb) in [#1022](https://github.com/umijs/mako/pull/1022)
* 修复 decorator visitor 应该在 preset env 之前运行 by [@stormslowly](https://github.com/stormslowly) in [#1176](https://github.com/umijs/mako/pull/1176)
* 修复 node 场景，添加需要 ignore 的报名 by [@sorrycc](https://github.com/sorrycc) in [#1182](https://github.com/umijs/mako/pull/1182)
* 修复 less ，在 Linux 上的 node 版本 < 20.12.0 时禁用并行 less loader by [@xusd320](https://github.com/xusd320) in [#1184](https://github.com/umijs/mako/pull/1184)
* 修复 less loader 中的 node 版本检查 by [@xusd320](https://github.com/xusd320) in [#1188](https://github.com/umijs/mako/pull/1188)
* 修复重新解析器以添加 ctxt by [@stormslowly](https://github.com/stormslowly) in [#1189](https://github.com/umijs/mako/pull/1189)
* 修复 px2rem min_pixel_value 应接受绝对值 by [@sorrycc](https://github.com/sorrycc) in [#1192](https://github.com/umijs/mako/pull/1192)
* 修复导出带数组参数的函数在 chrome 50 中的 swc bug by [@sorrycc](https://github.com/sorrycc) in [#1199](https://github.com/umijs/mako/pull/1199)
* 修复 watch 模式下的重复 assets 信息 by [@xusd320](https://github.com/xusd320) in [#1194](https://github.com/umijs/mako/pull/1194)
* 修复错误类型的 ctx by [@stormslowly](https://github.com/stormslowly) in [#1196](https://github.com/umijs/mako/pull/1196)
* 修复 rsc 支持 moduleIdStrategy hashed by [@sorrycc](https://github.com/sorrycc) in [#1201](https://github.com/umijs/mako/pull/1201)
* 修复 fix_helper_inject_position 支持导出 const 箭头函数 by [@sorrycc](https://github.com/sorrycc) in [#1207](https://github.com/umijs/mako/pull/1207)
* 修复 ts 中导出的命名空间类型未被剥离 by [@stormslowly](https://github.com/stormslowly) in [#1198](https://github.com/umijs/mako/pull/1198)
* 修复 watch 结果事件错误 panic by [@sorrycc](https://github.com/sorrycc) in [#1212](https://github.com/umijs/mako/pull/1212)
* 修复 watch 模式下添加动态依赖时应重新分组 by [@xusd320](https://github.com/xusd320) in [#1213](https://github.com/umijs/mako/pull/1213)
* 修复 inlineCSS 不工作 by [@stormslowly](https://github.com/stormslowly) in [#1211](https://github.com/umijs/mako/pull/1211)

## 0.4.17

`2024-05-16`

* 新增 watch=parent 支持 by [@sorrycc](https://github.com/sorrycc) in [#1151](https://github.com/umijs/mako/pull/1151)
* 新增 create-mako 包 by [@sorrycc](https://github.com/sorrycc) in [#1164](https://github.com/umijs/mako/pull/1164)
* 新增: 删除 output.ascii_only 配置项 by [@sorrycc](https://github.com/sorrycc) in [#1152](https://github.com/umijs/mako/pull/1152)
* 优化 less，支持 less 插件 by [@xusd320](https://github.com/xusd320) in [#1148](https://github.com/umijs/mako/pull/1148)
* 优化 less，兼容 ESM less 插件 by [@PeachScript](https://github.com/PeachScript) in [#1162](https://github.com/umijs/mako/pull/1162)
* 优化 stats.json，新增 modules 属性 中 by [@sorrycc](https://github.com/sorrycc) in [#1167](https://github.com/umijs/mako/pull/1167)
* 修复空 chunk 问题 by [@stormslowly](https://github.com/stormslowly) in [#1147](https://github.com/umijs/mako/pull/1147)
* 修复 ESM 和 require 混用问题 by [@stormslowly](https://github.com/stormslowly) in [#1154](https://github.com/umijs/mako/pull/1154)
* 修复生成空 chunk 时的 panic 问题 by [@xusd320](https://github.com/xusd320) in [#1135](https://github.com/umijs/mako/pull/1135)
* 修复 tree-shaking 导入的模块不返回 namespace 问题 by [@stormslowly](https://github.com/stormslowly) in [#1158](https://github.com/umijs/mako/pull/1158)
* 修复 在 bundless 模式下保留中文字符 by [@sorrycc](https://github.com/sorrycc) in [#1160](https://github.com/umijs/mako/pull/1160)
* 修复不正确的 chunk size map 问题 by [@xusd320](https://github.com/xusd320) in [#1161](https://github.com/umijs/mako/pull/1161)
* 修复 rsc sdk 中客户端 chunk 缺少兄弟模块的问题 by [@PeachScript](https://github.com/PeachScript) in [#1166](https://github.com/umijs/mako/pull/1166)

## 0.4.16

`2024-05-11`

> @alipay/bigfish@4.1.31

* 修复产物中文字符未转换成 unicode 问题 by [@sorrycc](https://github.com/sorrycc) in [#1146](https://github.com/umijs/mako/pull/1146)
* 修复模块合并优化时将忽略的模块合并导致未定义变量的问题 by [@stormslowly](https://github.com/stormslowly) in [#1149](https://github.com/umijs/mako/pull/1149)


## 0.4.15

`2024-05-10`

> @alipay/bigfish@4.1.29

* 优化 px2rem 支持 min_pixel_value 配置 by [@sorrycc](https://github.com/sorrycc) in [#1141](https://github.com/umijs/mako/pull/1141)
* 修复 px2rem 在使用属性选择器但没值时会 panic 的问题 by [@sorrycc](https://github.com/sorrycc) in [#1140](https://github.com/umijs/mako/pull/1140)
* 修复 node 补丁方案不支持 timers 的问题 by [@sorrycc](https://github.com/sorrycc) in [#1142](https://github.com/umijs/mako/pull/1142)

## 0.4.14

`2024-05-09`

> @alipay/bigfish@4.1.29

* 默认开启 concatenate modules by [@stormslowly](https://github.com/stormslowly) in [#1126](https://github.com/umijs/mako/pull/1126)
* 修复 chunk id 依赖顺序可能不稳定的问题 by [@stormslowly](https://github.com/stormslowly) in [#1117](https://github.com/umijs/mako/pull/1117)
* chore: add log for parallel generate by [@xusd320](https://github.com/xusd320) in [#1127](https://github.com/umijs/mako/pull/1127)
* 修复热更场景下，依赖类型变更时没有 re-group chunk 的问题 by [@xusd320](https://github.com/xusd320) in [#1124](https://github.com/umijs/mako/pull/1124)

## 0.4.13

`2024-05-06`

> @alipay/bigfish@4.1.29

* 新增支持通过 ?path 指定虚拟文件的路径 by [@stormslowly](https://github.com/stormslowly) in [#1102](https://github.com/umijs/mako/pull/1102)
* 新增全局 `__mako_chunk_load__` 方法 by [@sorrycc](https://github.com/sorrycc) in [#1111](https://github.com/umijs/mako/pull/1111)
* 优化 mako 命令行支持指定 mode by [@sorrycc](https://github.com/sorrycc) in [#1114](https://github.com/umijs/mako/pull/1114)
* 修复 concatenate inner global var conflict with other modules top level vars by [@stormslowly](https://github.com/stormslowly) in [#1100](https://github.com/umijs/mako/pull/1100)
* 修复 node polyfill 在 ident 简写场景不生效的问题 by [@stormslowly](https://github.com/stormslowly) in [#1104](https://github.com/umijs/mako/pull/1104)
* 修复 dev 阶段不输出 manifest 的问题 by [@sorrycc](https://github.com/sorrycc) in [#1106](https://github.com/umijs/mako/pull/1106)
* 修复 dev 阶段不输出 stats.json 的问题 by [@sorrycc](https://github.com/sorrycc) in [#1108](https://github.com/umijs/mako/pull/1108)
* 修复 cjs 构建的场景（for ssr） by [@Jinbao1001](https://github.com/Jinbao1001) in [#1109](https://github.com/umijs/mako/pull/1109)
* 重构移除 lazy_static by [@xusd320](https://github.com/xusd320) in [#1103](https://github.com/umijs/mako/pull/1103)
* 重构整体目录结构 by [@sorrycc](https://github.com/sorrycc) in [#1105](https://github.com/umijs/mako/pull/1105)
* 重构 okam 为 mako，同时公开 @alipay scope 的包到 @umijs 下 by [@sorrycc](https://github.com/sorrycc) in [#1113](https://github.com/umijs/mako/pull/1113)

## 0.4.12

`2024-04-28`

> @alipay/bigfish@4.1.28

* 修复 okam 包 package.json 中 bin 字段丢失的问题 by [@sorrycc](https://github.com/sorrycc) in [#1092](https://github.com/umijs/mako/pull/1092)
* 修复 runtime 在 node 场景下报错，让 css ensure 只在 browser 阶段才状态 by [@sorrycc](https://github.com/sorrycc) in [#1095](https://github.com/umijs/mako/pull/1095)
* 修复空 css chunk 不应该输出的问题 by [@xusd320](https://github.com/xusd320) in [#1097](https://github.com/umijs/mako/pull/1097)
* 修复 node 场景下不应该 load css 的问题（潜在的性能提升） by [@sorrycc](https://github.com/sorrycc) in [#1098](https://github.com/umijs/mako/pull/1098)
* 修复 concatenate 中 polyfill 没有在 inner 中被替换的问题 by [@stormslowly](https://github.com/stormslowly) in [#1099](https://github.com/umijs/mako/pull/1099)

## 0.4.11

`2024-04-25`

> @alipay/bigfish@4.1.27

* 新增 RSC 功能 by [@sorrycc](https://github.com/sorrycc) in [#1063](https://github.com/umijs/mako/pull/1063)
* 新增 RSC sdk  by [@sorrycc](https://github.com/sorrycc) in [#1072](https://github.com/umijs/mako/pull/1072)
* 新增 loader 返回参数增加 jsx 属性 by [@sorrycc](https://github.com/sorrycc) in [#1079](https://github.com/umijs/mako/pull/1079)
* 新增 experimental.webpackSyntaxValidate 配置 by [@sorrycc](https://github.com/sorrycc) in [#1080](https://github.com/umijs/mako/pull/1080)
* 新增 okam cli  by [@sorrycc](https://github.com/sorrycc) in [#1087](https://github.com/umijs/mako/pull/1087)
* 新增支持 css_rem 属性选择器 by [@LovePlayCode](https://github.com/LovePlayCode) in [#1059](https://github.com/umijs/mako/pull/1059)
* 新增支持伪类选择器 by [@LovePlayCode](https://github.com/LovePlayCode) in [#1061](https://github.com/umijs/mako/pull/1061)
* 修复 okam TS 类型问题 BuildParams by [@sorrycc](https://github.com/sorrycc) in [#1073](https://github.com/umijs/mako/pull/1073)
* 修复 mako 运行时全局变量无法获取 by [@PeachScript](https://github.com/PeachScript) in [#1082](https://github.com/umijs/mako/pull/1082)
* 修复 css 顺序不稳定 by [@xusd320](https://github.com/xusd320) in [#1085](https://github.com/umijs/mako/pull/1085)

## 0.4.10

`2024-04-16`

> @alipay/bigfish@4.1.26

* 新增 forkTSChecker 支持 by [@ctts](https://github.com/ctts) and @sorrycc in [#956](https://github.com/umijs/mako/pull/956)
* 优化 generate，让 entry 也并行执行，提速 10% by [@xusd320](https://github.com/xusd320) in [#1001](https://github.com/umijs/mako/pull/1001)
* 优化 px2rem 支持 selector_black_list 和 selector_white_list by [@LovePlayCode](https://github.com/LovePlayCode) and @sorrycc in [#1043](https://github.com/umijs/mako/pull/1043)
* 优化 less loader 实现，基于 worker，提升 20% by [@xusd320](https://github.com/xusd320) in [#1048](https://github.com/umijs/mako/pull/1048)
* 优化 importInfo，删除未使用的 specifier by [@goo-yyh](https://github.com/goo-yyh) in [#963](https://github.com/umijs/mako/pull/963)
* 优化 sourcemap 文件路径，把内部 runtime 代码移到 mako_internal 目录 by [@stormslowly](https://github.com/stormslowly) in [#1055](https://github.com/umijs/mako/pull/1055)
* 优化 ast to code 性能，dev 时并发执行 by [@xusd320](https://github.com/xusd320) in [#1053](https://github.com/umijs/mako/pull/1053)
* 重构 packages/mako 为入口 package by [@sorrycc](https://github.com/sorrycc) in [#1010](https://github.com/umijs/mako/pull/1010)
* 重构 @okamjs/okam 的实现，封装 less 等功能 by [@sorrycc](https://github.com/sorrycc) in [#1024](https://github.com/umijs/mako/pull/1024)
* 修复 concatenateModules 实现，var ident conflict with root's top vars by [@stormslowly](https://github.com/stormslowly) in [#1052](https://github.com/umijs/mako/pull/1052)
* 修复 dynamic_import_to_require 必须在 context_require 之后执行的问题 by [@sorrycc](https://github.com/sorrycc) in [#1038](https://github.com/umijs/mako/pull/1038)
* 修复 tree shaking 支持多个 declarator declare  by [@stormslowly](https://github.com/stormslowly) in [#1032](https://github.com/umijs/mako/pull/1032)
* 修复 provider，change unresolved indent syntax context to top level after it's been declared by [@stormslowly](https://github.com/stormslowly) in [#1027](https://github.com/umijs/mako/pull/1027)
* 修复 update 阶段的一个 unwrap() panic by [@sorrycc](https://github.com/sorrycc) in [#1004](https://github.com/umijs/mako/pull/1004)
* 修复 concatenateModule，treat module as external when it contains unsupported syntax by [@stormslowly](https://github.com/stormslowly) in [#1009](https://github.com/umijs/mako/pull/1009)

## 0.4.9

`2024-04-01`

> @alipay/bigfish@4.1.23

* 修复 chunk 优化中出现孤立 chunk 的问题 by [@Jinbao1001](https://github.com/Jinbao1001) in [#988](https://github.com/umijs/mako/pull/988)
* 修复 entry chunk hash 不稳定的问题 by [@xusd320](https://github.com/xusd320) in [#1003](https://github.com/umijs/mako/pull/1003)
* 修复 concatenateModules 无法合并多个外部模块的问题 [@stormslowly](https://github.com/stormslowly) in [#1005](https://github.com/umijs/mako/pull/1005)

## 0.4.8

`2024-03-23`

> @alipay/bigfish@4.1.17

* 新增 scope hoist 功能，配置开启 by [@stormslowly](https://github.com/stormslowly) in [#922](https://github.com/umijs/mako/pull/922)
* 修复 js hook 应该使用完整 path 的问题 by [@Jinbao1001](https://github.com/Jinbao1001) in [#987](https://github.com/umijs/mako/pull/987)
* 减少 tree shaking 阶段的性能开销 by [@xusd320](https://github.com/xusd320) in [#980](https://github.com/umijs/mako/pull/980)
* 删除 node_polyfill 里的正则以提升性能 by [@sorrycc](https://github.com/sorrycc) in [#998](https://github.com/umijs/mako/pull/998)
* 重构 generate cache hash 的处理 by [@xusd320](https://github.com/xusd320) in [#992](https://github.com/umijs/mako/pull/992)

## 0.4.7

`2024-03-22`

> @alipay/bigfish@4.1.17

* 修复 fast refresh 在函数内生成组件时的边界场景 by [@sorrycc](https://github.com/sorrycc) in [#972](https://github.com/umijs/mako/pull/972)
* 修复引用 assets 带 query 时的场景 by [@sorrycc](https://github.com/sorrycc) in [#975](https://github.com/umijs/mako/pull/975)

## 0.4.6

`2024-03-20`

> @alipay/bigfish@4.1.17

* 修复 resolve fragment 问题，支持 a#b.ts 的场景 by [@sorrycc](https://github.com/sorrycc) in [#966](https://github.com/umijs/mako/pull/966)

## 0.4.5

`2024-03-20`

> @alipay/bigfish@4.1.17

* 重构 build 部分的代码 by [@sorrycc](https://github.com/sorrycc) in [#923](https://github.com/umijs/mako/pull/923)
* 新增 HMR Fast Refresh 支持匿名函数的场景 by [@JackGuiYang12](https://github.com/JackGuiYang12) in [#947](https://github.com/umijs/mako/pull/947)
* 新增 inline_css 配置，实现类 style-loader 的能力 by [@sorrycc](https://github.com/sorrycc) in [#957](https://github.com/umijs/mako/pull/957)
* 优化 rayon 使用，generate 复用 build 阶段的 rayon 线程 by [@xusd320](https://github.com/xusd320) in [#959](https://github.com/umijs/mako/pull/959)
* 优化 minifish inject 功能，支持 include 配置项 by [@stormslowly](https://github.com/stormslowly) in [#930](https://github.com/umijs/mako/pull/930)
* 修复 async chunk 不应该拆分 root module by [@PeachScript](https://github.com/PeachScript) in [#929](https://github.com/umijs/mako/pull/929)
* 修复 css url() 应该支持 # 前缀 by [@sorrycc](https://github.com/sorrycc) in [#949](https://github.com/umijs/mako/pull/949)
* 修复 async module 的实现 by [@stormslowly](https://github.com/stormslowly) in [#943](https://github.com/umijs/mako/pull/943)
* 修复 js 和 css resolve 依赖时对 # fragment 的支持 by [@sorrycc](https://github.com/sorrycc) in [#952](https://github.com/umijs/mako/pull/952)
* 修复非 ascii 路径的支持，比如空格和中文 by [@sorrycc](https://github.com/sorrycc) in [#958](https://github.com/umijs/mako/pull/958)
* 修复 ignored 模块应该被编译成空的 es 模块 by [@xusd320](https://github.com/xusd320) in [#946](https://github.com/umijs/mako/pull/946)
* 修复 context module 场景下 async import 应该被拆分的问题 by [@xusd320](https://github.com/xusd320) in [#940](https://github.com/umijs/mako/pull/940)
* 修复 sync chunk 的 stats 信息 by [@PeachScript](https://github.com/PeachScript) in [#928](https://github.com/umijs/mako/pull/928)

## 0.4.4

`2024-02-29`

> @alipay/bigfish@4.1.17

- 修复在 call_expr 中的动态 require/import 未被正常转换的问题 by [@PeachScript](https://github.com/PeachScript) in [#898](https://github.com/umijs/mako/pull/898)
- 兼容 extraBabelPlugins: ['@emotion'] 插件配置 by [@sorrycc](https://github.com/sorrycc) in [#908](https://github.com/umijs/mako/pull/908)
- 使用更高效的内存分配器（mimalloc-rust、tikv-jemallocator），m1 pro yuyanAssets build 稳定提升 2500ms 左右 by [@xusd320](https://github.com/xusd320) in [#912](https://github.com/umijs/mako/pull/912)
- 优化 external 特性中正则表达式实例化的开销，m1 pro yuyanAssets build 稳定提升 3900ms 左右 by [@PeachScript](https://github.com/PeachScript) in [#916](https://github.com/umijs/mako/pull/916)
- 调用 onBuildComplete hook 时传入全量的 stats compilation 数据 by [@PeachScript](https://github.com/PeachScript) in [#917](https://github.com/umijs/mako/pull/917)
- 从 nodejs-resolver 切换至 oxc_resolver by [@xusd320](https://github.com/xusd320) in [#919](https://github.com/umijs/mako/pull/919)

## 0.4.3

`2024-02-01`

> @alipay/bigfish@4.1.17

- 修复 skipModules 在边界情况下找错导出来源的问题 by [@stormslowly](https://github.com/stormslowly) in [#906](https://github.com/umijs/mako/pull/906)
- 回滚 SWC 升级的 PR [#876](https://github.com/umijs/mako/pull/876) by [@stormslowly](https://github.com/stormslowly) in [#907](https://github.com/umijs/mako/pull/907)

## 0.4.2

`2024-01-31`

> @alipay/bigfish@4.1.17

- 修复 lessLoader.modifyVars dev 环境不生效的问题 by [@sorrycc](https://github.com/sorrycc) in [#900](https://github.com/umijs/mako/pull/900)
- 修复 node binding 因为 stout/stderr 模式不匹配导致的 OS error 35 by [@sorrycc](https://github.com/sorrycc) in [#901](https://github.com/umijs/mako/pull/901)
- 修复 package.json 中 sideEffects 配置为相对路径时，sideEffects 匹配错误的 bug by [@stormslowly](https://github.com/stormslowly) in [#902](https://github.com/umijs/mako/pull/902)

## 0.4.1

`2024-01-30`

> @alipay/bigfish@4.1.17

* 新增 HMR 支持 link 的 npm 包的调试 by [@zhangpanweb](https://github.com/zhangpanweb) in [#864](https://github.com/umijs/mako/pull/864)
* 新增支持类似 raw-loader 的能力，通过加 ?raw query 开启 by [@ctts](https://github.com/ctts) in [#877](https://github.com/umijs/mako/pull/877)
* 新增 cjs 输出配置 by [@sorrycc](https://github.com/sorrycc) in [#886](https://github.com/umijs/mako/pull/886)
* 新增 async script 的 preload 支持 by [@PeachScript](https://github.com/PeachScript) in [#895](https://github.com/umijs/mako/pull/895)
* 新增 emit_assets 和 css_modules_export_only_locales 配置 by [@sorrycc](https://github.com/sorrycc) in [#890](https://github.com/umijs/mako/pull/890)
* 升级 swc 到 86 by [@goo-yyh](https://github.com/goo-yyh) in [#876](https://github.com/umijs/mako/pull/876)
* 优化 node 场景下对 __dirname 和 __filename 的支持 by [@zhangpanweb](https://github.com/zhangpanweb) in [#885](https://github.com/umijs/mako/pull/885)
* 优化 platform: node 场景下的 code splitting 支持 by [@sorrycc](https://github.com/sorrycc) in [#887](https://github.com/umijs/mako/pull/887)
* 优化检测变量是否声明的方法，以提升速度 by [@zhangpanweb](https://github.com/zhangpanweb) in [#897](https://github.com/umijs/mako/pull/897)
* 优化 stats 信息，添加 siblings 和 origins 信息 by [@PeachScript](https://github.com/PeachScript) in [#893](https://github.com/umijs/mako/pull/893)
* 重构 emotion 插件的实现 by [@zhangpanweb](https://github.com/zhangpanweb) in [#884](https://github.com/umijs/mako/pull/884)

## 0.4.0

`2024-01-18`

> @alipay/bigfish@4.1.17

* 新增 react 配置项，支持不同的 react runtime 参数配置 by [@sorrycc](https://github.com/sorrycc) in [#872](https://github.com/umijs/mako/pull/872)
* 新增 mako.config.json 中有错误时，输出友好提示 by [@sorrycc](https://github.com/sorrycc) in [#875](https://github.com/umijs/mako/pull/875)
* 修复 HMR 无法从文件错误中恢复的问题 by [@sorrycc](https://github.com/sorrycc) in [#863](https://github.com/umijs/mako/pull/863)
* 修复 Less 参数取值优先读取 modifyVars 字段，其次 theme by [@sorrycc](https://github.com/sorrycc) in [#874](https://github.com/umijs/mako/pull/874)
* 修复 style 文件导入语句未删除的问题 by [@stormslowly](https://github.com/stormslowly) in [#869](https://github.com/umijs/mako/pull/869)
* 修复 skipModule 优化时丢失 default 导入的副作用 [@stormslowly](https://github.com/stormslowly) in [#862](https://github.com/umijs/mako/pull/862)

## 0.3.1

`2024-01-11`

> @alipay/bigfish@4.1.15

* 修复产物中模块 ID 的注释被 glob 表达式意外切断导致运行报错的问题 by [@PeachScript](https://github.com/PeachScript) in [#856](https://github.com/umijs/mako/pull/856)
* 优化内置插件钩子及暴露的 js hooks by [@sorrycc](https://github.com/sorrycc) in [#855](https://github.com/umijs/mako/pull/855)

## 0.2.3

`2024-01-10`

> @alipay/bigfish@4.1.14

* 暂时关闭 skipModules 优化 by [@stormslowly](https://github.com/stormslowly) in [#854](https://github.com/umijs/mako/pull/854)

## 0.2.2

`2024-01-09`

> @alipay/bigfish@4.1.14

* 修复 import/export 语句中使用 `as` 对同一导出使用多次导致的变量 undefined 问题 by [@stormslowly](https://github.com/stormslowly) in [#850](https://github.com/umijs/mako/pull/850)
* 修复 dev 启动时缺少的依赖在补齐后仍然构建失败的问题 by [@zhangpanweb](https://github.com/zhangpanweb) in [#845](https://github.com/umijs/mako/pull/845)
* 修复 less 文件中引入相对路径的 css 可能解析失败的问题 by [@sorrycc](https://github.com/sorrycc) in [#844](https://github.com/umijs/mako/pull/844)
* 优化产物生成，在不压缩时保留注释便于开发者排查问题 by [@sorrycc](https://github.com/sorrycc) in [#848](https://github.com/umijs/mako/pull/848)
* 优化产物生成，为模块声明添加 ID 注释便于开发者排查问题 by [@sorrycc](https://github.com/sorrycc) in [#849](https://github.com/umijs/mako/pull/849)

## 0.2.1

`2024-01-04`

> @alipay/bigfish@4.1.14

* 修复模块中对相同模块同时使用 import 和 require 时，skipModules 优化时误删依赖模块的问题 by [@stormslowly](https://github.com/stormslowly) in [#842](https://github.com/umijs/mako/pull/842)

## 0.2.0

`2024-01-04`

> @alipay/bigfish@4.1.14

* BREAK CHANGE：调整和合并配置项 by [@PeachScript](https://github.com/PeachScript) in [#837](https://github.com/umijs/mako/pull/837)
* 优化 optimizePackageImports 功能，支持缓存、export * 和深度桶文件 by [@sorrycc](https://github.com/sorrycc) in [#810](https://github.com/umijs/mako/pull/810)
* 优化 tree shaking，实现 skip modules 以进一步减少产物尺寸 by [@stormslowly](https://github.com/stormslowly) in [#807](https://github.com/umijs/mako/pull/807)
* 优化 stats，添加 entry 信息 by [@PeachScript](https://github.com/PeachScript) in [#823](https://github.com/umijs/mako/pull/823) #829
* 修复 runtime 里一处 es5 不兼容的代码 in [#830](https://github.com/umijs/mako/pull/830)
* 修复 cjs 模块被插入 esm helpers 导致变成 esm 模块的问题 by [@stormslowly](https://github.com/stormslowly) in [#831](https://github.com/umijs/mako/pull/831)
* 修复 optimizePackageImports default export 的处理 by [@zhangpanweb](https://github.com/zhangpanweb) in [#832](https://github.com/umijs/mako/pull/832)
* 优化 skip modules 的性能问题 by [@stormslowly](https://github.com/stormslowly) in [#826](https://github.com/umijs/mako/pull/826)
* 优化一处细节性能问题 by [@stormslowly](https://github.com/stormslowly) in [#835](https://github.com/umijs/mako/pull/835)

## 0.1.15

`2023-12-18`

> @alipay/bigfish@4.1.13

* 新增 支持显示 less 代码 sourcemap by [@jiesia](https://github.com/jiesia) in [#755](https://github.com/umijs/mako/pull/775)
* 修复 require context 不支持后缀 require 的问题 by [@PeachScript](https://github.com/PeachScript) in [#806](https://github.com/umijs/mako/pull/806)
* 修复 entry chunk 和 vendor chunk 中模块重复的问题 by [@PeachScript](https://github.com/PeachScript) in [#809](https://github.com/umijs/mako/pull/809)
* 修复 dynamicImportToRequire 开启后遗漏动态引用模块的问题 by [@stormslowly](https://github.com/stormslowly)  in [#811](https://github.com/umijs/mako/pull/811)

## 0.1.14

`2023-12-18`

> @alipay/bigfish@4.1.12

* 优化 chunk cache 在 build 时默认不使用 cache by [@zhangpanweb](https://github.com/zhangpanweb) in [#800](https://github.com/umijs/mako/pull/800)
* 修复由于 hashed id 策略导致的 build 后 chunk 可能加载失败的问题 by [@PeachScript](https://github.com/PeachScript) in [#805](https://github.com/umijs/mako/pull/805)
* 修复 try 语句下的 require 失败时的报错处理，在 dev 时也不在命令行报错 by [@sorrycc](https://github.com/sorrycc) in [#803](https://github.com/umijs/mako/pull/803)
* 修复 react refresh runtime 代码不应该出现在 platform:node 的 HMR 场景下 by [@sorrycc](https://github.com/sorrycc) in [#802](https://github.com/umijs/mako/pull/802)
* 修复 devtool 为 none 时产物为空 by [@zhangpanweb](https://github.com/zhangpanweb) in [#801](https://github.com/umijs/mako/pull/801)
* 修复非 entry chunk 可能加载失败的问题 by [@PeachScript](https://github.com/PeachScript) in [#798](https://github.com/umijs/mako/pull/798)
* 重构 chunks 收集算法以避免潜在的 stack overflow 问题 by [@PeachScript](https://github.com/PeachScript) in [#799](https://github.com/umijs/mako/pull/799)

## 0.1.12

`2023-12-14`

> @alipay/bigfish@4.1.9

* 新增多 entry 支持 shared chunk by [@PeachScript](https://github.com/PeachScript) in [#789](https://github.com/umijs/mako/pull/789)
* 修复 SWC Helper 在 module_id_strategy 为 hashed 时注入失败的问题 by [@sorrycc](https://github.com/sorrycc) in [#797](https://github.com/umijs/mako/pull/797)
* 修复 HMR 时优化 chunk 时可能会死锁的问题 by [@PeachScript](https://github.com/PeachScript) in [#795](https://github.com/umijs/mako/pull/795)

## 0.1.11

`2023-12-14`

> @alipay/bigfish@4.1.8

* 修复 CSS 热更在开启 runtimePublicPath 且最终值包含 origin 时失效的问题 by [@PeachScript](https://github.com/PeachScript) in [#768](https://github.com/umijs/mako/pull/768)
* 修复 require 动态目录时文件夹带假后缀名会编译失败的问题 by [@PeachScript](https://github.com/PeachScript) in [#778](https://github.com/umijs/mako/pull/778)
* 修复 CSS 文件内容相同时 chunk 可能丢失的问题 by [@stormslowly](https://github.com/stormslowly) in [#781](https://github.com/umijs/mako/pull/781)
* 优化 runtime 的尺寸，能力按需生成 by [@sorrycc](https://github.com/sorrycc) in [#767](https://github.com/umijs/mako/pull/767)
* 优化 chunk 加载及注册逻辑，支持非 entry chunk 先于 entry chunk 加载 by [@PeachScript](https://github.com/PeachScript) in [#783](https://github.com/umijs/mako/pull/783)

## 0.1.10

`2023-12-08`

> @alipay/bigfish@4.1.7

* 调整 swc 相应依赖替换成 swc_core by [@goo-yyh](https://github.com/goo-yyh) in [#765](https://github.com/umijs/mako/pull/765)
* 调整 tree-shaking 增加对变量声明语句副作用的判定 by [@stormslowly](https://github.com/stormslowly) in [#763](https://github.com/umijs/mako/pull/763)
* 修复 node binding TypeScript 定义 by [@stormslowly](https://github.com/stormslowly) in [#761](https://github.com/umijs/mako/pull/761)

## 0.1.9

`2023-12-07`

> @alipay/bigfish@4.1.6

- 新增 require 动态字符串的支持，比如 `require('./i18n' + lang)` by [@PeachScript](https://github.com/PeachScript) in [#747](https://github.com/umijs/mako/pull/747)
- 调整 tree-shaking，优化对 side effects 的处理 by [@stormslowly](https://github.com/stormslowly) in [#725](https://github.com/umijs/mako/pull/725)
- 重构 watch、dev 和 update 逻辑，增加 debounce，支持 git checkout 一次触发多次修改的问题 by [@sorrycc](https://github.com/sorrycc) in [#744](https://github.com/umijs/mako/pull/744)
- 修复 import() 内容为空时会 panic 报错的问题 by [@sorrycc](https://github.com/sorrycc) in [#743](https://github.com/umijs/mako/pull/743)
- 修复 require(css_file) 被处理成 css modules 的问题 by [@sorrycc](https://github.com/sorrycc) in [#751](https://github.com/umijs/mako/pull/751)
- 修复 node 补丁方案不支持类 fs/promise 引用的问题 by [@sorrycc](https://github.com/sorrycc) in [#746](https://github.com/umijs/mako/pull/746)
- 修复 import() 动态加载 CSS 不生效的问题 by [@jiesia](https://github.com/jiesia) in [#756](https://github.com/umijs/mako/pull/756)
- 修复 worker 里不支持 import() 动态加载的问题 by [@jiesia](https://github.com/jiesia) in [#755](https://github.com/umijs/mako/pull/755)
- 修复 HMR 时有几率触发 process undefined 的问题 by [@sorrycc](https://github.com/sorrycc) in [#741](https://github.com/umijs/mako/pull/741)
- 修复 external 配置格式判断逻辑 by [@PeachScript](https://github.com/PeachScript) in [#735](https://github.com/umijs/mako/pull/735)
- 修复 Minifish 的 inject 功能支持配置 preferRequire 和 插入代码顺序根据根据插入变量出现顺序排序 by [@stormslowly](https://github.com/stormslowly) in [#731](https://github.com/umijs/mako/pull/731) #734

## 0.1.5

`2023-11-28`

> @alipay/bigfish@4.1.3

- 添加 flexBugs 配置项，并在 umi 场景下默认开启 by [@PeachScript](https://github.com/PeachScript) in [#728](https://github.com/umijs/mako/pull/728)
- 修复 okam node 包的 dts 问题 by [@stormslowly](https://github.com/stormslowly) in [#726](https://github.com/umijs/mako/pull/726)

## 0.1.3

`2023-11-27`

> @alipay/bigfish@4.1.2

- 修复合并到 common 的 async chunk 在生成 ensure 语句时数据错误导致加载失败的问题 by [@PeachScript](https://github.com/PeachScript) in [#712](https://github.com/umijs/mako/pull/712)
- 修复 require.loadScript 没有替换成 `__mako__require__` 的问题 by [@stormslowly](https://github.com/stormslowly) in [#715](https://github.com/umijs/mako/pull/715)
- 添加 node api 的 dts by [@stormslowly](https://github.com/stormslowly) in [#716](https://github.com/umijs/mako/pull/716)

## 0.1.0

`2023-11-23`

> @alipay/bigfish@4.1.0

- 新增 Emotion 支持 by [@zhangpanweb](https://github.com/zhangpanweb) in [#694](https://github.com/umijs/mako/pull/694)
- 提升 generate chunks 的性能，m1 yuyanAssets build 稳定提升 200ms 左右 by [@sorrycc](https://github.com/sorrycc) in [#709](https://github.com/umijs/mako/pull/709)
- 提升 generate 阶段 transform 的性能，m1 yuyanAssets 此步骤耗时从 3431ms 降到 1019ms by [@sorrycc](https://github.com/sorrycc) in [#707](https://github.com/umijs/mako/pull/707)
- 修复 window.require 不应该被替换成 `window.__mako_require__` 的问题 by [@jiesia](https://github.com/jiesia) in [#708](https://github.com/umijs/mako/pull/708)
- 修复 transform_in_generate 多线程 transform 时子线程错误没有被处理的问题 by [@sorrycc](https://github.com/sorrycc) in [#710](https://github.com/umijs/mako/pull/710)
- 捕获更多不支持的 loader 语法，比如 file-loader?esModule=false!./src-noconflict/theme-kr_theme.js by [@sorrycc](https://github.com/sorrycc) in [#704](https://github.com/umijs/mako/pull/704)
