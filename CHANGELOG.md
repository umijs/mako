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
