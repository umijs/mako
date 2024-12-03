## 0.9.7

`2024-11-25`

- fix: devserver response header add cacheControl no-cache by [@Jinbao1001](https://github.com/Jinbao1001) in [#1692](https://github.com/umijs/mako/pull/1692) [#1699](https://github.com/umijs/mako/pull/1699)
- fix(ssu): in case external not available to all entries by [@stormslowly](https://github.com/stormslowly) in [#1698](https://github.com/umijs/mako/pull/1698)

## 0.9.6

`2024-11-14`

- feat(ssu): handle dependence changing while watching by [@stormslowly](https://github.com/stormslowly) in [#1690](https://github.com/umijs/mako/pull/1690)
- feat: move ensure runtime to entry  by [@stormslowly](https://github.com/stormslowly) in [#1660](https://github.com/umijs/mako/pull/1660)
- feat: keep unresolved nodejs require by [@xusd320](https://github.com/xusd320) in [#1689](https://github.com/umijs/mako/pull/1689)
- fix: pnpm workspace watch too many files by [@Jinbao1001](https://github.com/Jinbao1001) in [#1684](https://github.com/umijs/mako/pull/1684)
- fix: ts annotated declare variable treat as top level variable by [@stormslowly](https://github.com/stormslowly) in [#1682](https://github.com/umijs/mako/pull/1682)

## 0.9.5

`2024-11-07`

- fix: skip module should skip async module by [@Jinbao1001](https://github.com/Jinbao1001) in [#1667](https://github.com/umijs/mako/pull/1662)

## 0.9.4

`2024-11-04`

- feat: enable magicComment features by default by [@xusd320](https://github.com/xusd320) in [#1667](https://github.com/umijs/mako/pull/1667)
- feat(bundler-mako): add moduleIdStrategy to supportMakoConfigKeys by [@Jinbao1001](https://github.com/Jinbao1001) in [#1664](https://github.com/umijs/mako/pull/1664)
- feat: compatible codeSplitting config with umi by [@xusd320](https://github.com/xusd320) in [#1669](https://github.com/umijs/mako/pull/1669)
- fix: hmr with magic comment chunk name by [@xusd320](https://github.com/xusd320) in [#1663](https://github.com/umijs/mako/pull/1663)
- fix: async module in circular dependence by [@stormslowly](https://github.com/stormslowly) in [#1659](https://github.com/umijs/mako/pull/1659)

## 0.9.3

`2024-10-25`

- feat: add `buildEnd` plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1644](https://github.com/umijs/mako/pull/1644)
- feat: add `enforce` plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1646](https://github.com/umijs/mako/pull/1646)
- feat: add `writeBundle` plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1650](https://github.com/umijs/mako/pull/1650)
- feat: add `watchChanges` plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1651](https://github.com/umijs/mako/pull/1651)
- fix: mako on windows don't work by [@sorrycc](https://github.com/sorrycc) in [#1652](https://github.com/umijs/mako/pull/1652)
- fix: devtool sourcemap explosion in windows by [@sorrycc](https://github.com/sorrycc) in [#1653](https://github.com/umijs/mako/pull/1653)
- fix: should not re-group when span changed by [@xusd320](https://github.com/xusd320) in [#1654](https://github.com/umijs/mako/pull/1654)
- fix: umd should be import as cjs by [@Jinbao1001](https://github.com/Jinbao1001) in [#1642](https://github.com/umijs/mako/pull/1642)
- fix: add `process.env.SOCKET_SERVER` define to prevent process polyfilll by [@stormslowly](https://github.com/stormslowly) in [#1655](https://github.com/umijs/mako/pull/1655)

## 0.9.2

`2024-10-16`

- feat: support webpackIgnore and makoIgnore magic comment by [@sorrycc](https://github.com/sorrycc) in [#1636](https://github.com/umijs/mako/pull/1636)
- feat: add transform plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1637](https://github.com/umijs/mako/pull/1637)
- feat: add transformInclude plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1639](https://github.com/umijs/mako/pull/1639)
- fix: import namespace optimize panic with nested for of stmt by [@stormslowly](https://github.com/stormslowly) in [#1640](https://github.com/umijs/mako/pull/1640)
- fix: duplicate\_package\_checker panic when package.json has no version field by [@sorrycc](https://github.com/sorrycc) in [#1634](https://github.com/umijs/mako/pull/1634)

## 0.9.0

`2024-10-14`

- feat: add loadInclude plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1630](https://github.com/umijs/mako/pull/1630)
- feat: add { isEntry } for resolve\_id plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1631](https://github.com/umijs/mako/pull/1631)
- feat: upgrade swc\_core to 0.101.x by [@stormslowly](https://github.com/stormslowly) in [#1444](https://github.com/umijs/mako/pull/1444)
- fix: hash not stable caused by module concatenate by [@Jinbao1001](https://github.com/Jinbao1001) in [#1610](https://github.com/umijs/mako/pull/1610)

## 0.8.15

`2024-10-10`

* feat: disable webp to base64 by [@Jinbao1001](https://github.com/Jinbao1001) in [#1602](https://github.com/umijs/mako/pull/1602)
* feat: add resolve_id plugin hook by [@sorrycc](https://github.com/sorrycc) in [#1625](https://github.com/umijs/mako/pull/1625)
* refactor: napi threadsafe function by [@xusd320](https://github.com/xusd320) in [#1608](https://github.com/umijs/mako/pull/1608)
* refactor: config codes organization by [@xusd320](https://github.com/xusd320) in [#1618](https://github.com/umijs/mako/pull/1618)
* fix(bundler-mako): experimental config should be merged deeply by [@sorrycc](https://github.com/sorrycc) in [#1617](https://github.com/umijs/mako/pull/1617)
* fix: clickToComponent don't work by [@sorrycc](https://github.com/sorrycc) in [#1620](https://github.com/umijs/mako/pull/1620)
* fix: duplicate_package_checker panic when no package.json is supplied by [@sorrycc](https://github.com/sorrycc) in [#1621](https://github.com/umijs/mako/pull/1621)
* fix: file_stem index out of bound by [@Jinbao1001](https://github.com/Jinbao1001) in [#1623](https://github.com/umijs/mako/pull/1623)

## 0.8.14

`2024-09-25`

* fix: bundler-mako dev server load chunks failed with 504 error code by [@stormslowly](https://github.com/stormslowly) in [#1612](https://github.com/umijs/mako/pull/1612)

## 0.8.13

`2024-09-23`

* fix: chunk_loading_global  by [@xusd320](https://github.com/xusd320) in [#1590](https://github.com/umijs/mako/pull/1590)
* fix: devServer put static serve proxy after umi proxy middleware  by [@whyer11](https://github.com/whyer11) in [#1558](https://github.com/umijs/mako/pull/1558)
* revert: import namespace optimize  by [@stormslowly](https://github.com/stormslowly) in [#1606](https://github.com/umijs/mako/pull/1606)

## 0.8.12

`2024-09-13`

* fix(tree-shaking): detect export var side effects by [@stormslowly](https://github.com/stormslowly) in [#1579](https://github.com/umijs/mako/pull/1579)
* fix: bad output when chunk_loading_global containing quotation mark by [@xusd320](https://github.com/xusd320) in [#1582](https://github.com/umijs/mako/pull/1582)
* chore: ➕ add a subdot cli tool script for debug  module/chunk graph  by [@stormslowly](https://github.com/stormslowly) in [#1585](https://github.com/umijs/mako/pull/1585)
* fix(win): copy don't work under windows by [@sorrycc](https://github.com/sorrycc) in [#1587](https://github.com/umijs/mako/pull/1587)
* fix(win): module id should be win_pathed by [@sorrycc](https://github.com/sorrycc) in [#1588](https://github.com/umijs/mako/pull/1588)
* feat(tree-shaking): optimize import namespace used all exports to partial used of source modules by [@stormslowly](https://github.com/stormslowly) in [#1584](https://github.com/umijs/mako/pull/1584)
* fix: merge mako config by [@hualigushi](https://github.com/hualigushi) in [#1578](https://github.com/umijs/mako/pull/1578)
* fix:clear deps should not panic when module not found by [@Jinbao1001](https://github.com/Jinbao1001) in [#1581](https://github.com/umijs/mako/pull/1581)
* Revert "fix: merge mako config" by [@stormslowly](https://github.com/stormslowly) in [#1589](https://github.com/umijs/mako/pull/1589)
* fix: watch too many file error by [@Jinbao1001](https://github.com/Jinbao1001) in [#1550](https://github.com/umijs/mako/pull/1550)
* feat: support numeric module Id by [@Jinbao1001](https://github.com/Jinbao1001) in [#1561](https://github.com/umijs/mako/pull/1561)

## 0.8.11

`2024-09-10`

* fix: env_replacer should not replace user defined variable in scope by [@xusd320](https://github.com/xusd320) in [#1577](https://github.com/umijs/mako/pull/1577)

## 0.8.10

`2024-09-05`

* feat: support linux arm64 gnu by [@xusd320](https://github.com/xusd320) in [#1570](https://github.com/umijs/mako/pull/1570)
* fix: parse_path failed under windows by [@sorrycc](https://github.com/sorrycc) in [#1571](https://github.com/umijs/mako/pull/1571)
* feat: support runtime global module registry by [@xusd320](https://github.com/xusd320) in [#1574](https://github.com/umijs/mako/pull/1574)
* feat: add bundle for windows  by [@sorrycc](https://github.com/sorrycc) in [#1575](https://github.com/umijs/mako/pull/1575)

## 0.8.8

`2024-09-05`

* perf: group chunks with right first dfs by [@xusd320](https://github.com/xusd320) in [#1554](https://github.com/umijs/mako/pull/1554)
* refactor: unify base64 utils by [@xusd320](https://github.com/xusd320) in [#1557](https://github.com/umijs/mako/pull/1557)
* Revert "refactor: Unify the static server in bundler-mako and devServer" by [@stormslowly](https://github.com/stormslowly) in [#1556](https://github.com/umijs/mako/pull/1556)
* fix: define env by [@xusd320](https://github.com/xusd320) in [#1551](https://github.com/umijs/mako/pull/1551)
* fix: When hmr=none,mako does not take effect by [@Wu-kung](https://github.com/Wu-kung) in [#1552](https://github.com/umijs/mako/pull/1552)
* fix: 🐛 concatenated module exported namespace should sort key  by [@stormslowly](https://github.com/stormslowly) in [#1564](https://github.com/umijs/mako/pull/1564)
* fix: camel case in napi binding params by [@xusd320](https://github.com/xusd320) in [#1565](https://github.com/umijs/mako/pull/1565)

## 0.8.7

`2024-08-30`

* refactor: 🎨 assign tpl's span to literal string by [@stormslowly](https://github.com/umijs/mako/pull/1529)
* perf: reapply pr 1509 and sourcemap missing when chain_map is empty by [@xusd320](https://github.com/umijs/mako/pull/1542)
* chore: ✨ strip span when parsing define's expression by [@stormslowly](https://github.com/umijs/mako/pull/1540)
* feat: support to control crossorigin for async chunk scripts and links by [@PeachScript](https://github.com/umijs/mako/)pull/1539
* refactory:  in str-impl chunk generate, remove cm when  merge_code_and_sourcemap by [@stormslowly](https://github.com/umijs/mako/pull/1541)
* fix: entry support sub paths by [@sorrycc](https://github.com/umijs/mako/pull/1547)
* fix: filename too long when use pnpm by [@Jinbao1001](https://github.com/umijs/mako/pull/1421)
* refactor: Unify the static server in bundler-mako and devServer by [@whyer11](https://github.com/umijs/mako/pull/1468)
* feat: #1491 add duplicate package checker plugin by [@jeasonnow](https://github.com/umijs/mako/pull/1496)
* fix: #1478 support react class-component hot-update by [@jeasonnow](https://github.com/umijs/mako/pull/1489)
* fix(plugin:emotion): panic when target to chrome 40 by [@stormslowly](https://github.com/umijs/mako/pull/1527)

## 0.8.6

`2024-08-26`

- revert: [#1538](https://github.com/umijs/mako/pull/1475) and [#1538](https://github.com/umijs/mako/pull/1509) by [@xusd320](https://github.com/xusd320) in [#1538](https://github.com/umijs/mako/pull/1538)


## 0.8.5

`2024-08-26`

- feat: support aarch64-unknown-linux-musl by [@stormslowly](https://github.com/stormslowly) in [#1535](https://github.com/umijs/mako/pull/1535)

## 0.8.4

`2024-08-23`

- fix: should not alias define XXX to process.env.XXX by [@xusd320](https://github.com/xusd320) in [#1504](https://github.com/umijs/mako/pull/1526)

## 0.8.3

`2024-08-22`

- fix: wrong file extension for map file paths in stat.json by [@stormslowly](https://github.com/stormslowly) in [#1506](https://github.com/umijs/mako/pull/1506)
- fix: resolve failed when package use `node` as key by [@sorrycc](https://github.com/sorrycc) in [#1516](https://github.com/umijs/mako/pull/1516)
- perf: merge source map, speed up generation by 800% by [@xusd320](https://github.com/xusd320) in [#1509](https://github.com/umijs/mako/pull/1509)
- perf: optimize group_chunks, speed up group_chunks by 500% by [@xusd320](https://github.com/xusd320) in [#1475](https://github.com/umijs/mako/pull/1475)
- refactor: improve regex convention for px2rem config by [@xiaohuoni](https://github.com/xiaohuoni) in [#1469](https://github.com/umijs/mako/pull/1469)
- refactor: improve behavior of define config by [@xusd320](https://github.com/xusd320) in [#1505](https://github.com/umijs/mako/pull/1505)

## 0.8.2

`2024-08-16`

- Revert "refactor: define env ([#1499](https://github.com/umijs/mako/pull/1499))" by [@stormslowly](https://github.com/stormslowly) in [#1504](https://github.com/umijs/mako/pull/1504)

## 0.8.1

`2024-08-16`

- feat: support progress by [@xierenyuan](https://github.com/xierenyuan) in [#1466](https://github.com/umijs/mako/pull/1466)
- refactor: ✨ disable emotion source map in prod by [@stormslowly](https://github.com/stormslowly) in [#1494](https://github.com/umijs/mako/pull/1494)
- refactor: define env by [@xusd320](https://github.com/xusd320) in [#1499](https://github.com/umijs/mako/pull/1499)
- fix: sass plugin support `.scss` extension by [@jeasonnow](https://github.com/jeasonnow) in [#1482](https://github.com/umijs/mako/pull/1482)
- fix: try require should support return stmt by [@sorrycc](https://github.com/sorrycc) in [#1488](https://github.com/umijs/mako/pull/1488)
- fix: hashed chunk file name starts with underscore by [@stormslowly](https://github.com/stormslowly) in [#1498](https://github.com/umijs/mako/pull/1498)
- fix: no unnecessary chunk group in update by [@stormslowly](https://github.com/stormslowly) in [#1503](https://github.com/umijs/mako/pull/1503)
- fix: support require css modules by [@bytemain](https://github.com/bytemain) in [#1501](https://github.com/umijs/mako/pull/1501)

## 0.8.0

`2024-08-08`

* [Breaking Change] refactor: not write stats.json anymore by [@xusd320](https://github.com/xusd320) in [#1485](https://github.com/umijs/mako/pull/1485)
* feat: less support "globalVars" by [@gin-lsl](https://github.com/gin-lsl) in [#1465](https://github.com/umijs/mako/pull/1465)
* feat(bundler-mako): generate dynamicImportToRequire from babel and webpack config by [@PeachScript](https://github.com/PeachScript) in [#1479](https://github.com/umijs/mako/pull/1479)
* refactor: avoid underscore prefix for chunk file name by [@PeachScript](https://github.com/PeachScript) in [#1471](https://github.com/umijs/mako/pull/1471)

## 0.7.9

`2024-08-01`

- feat: generate_end with stats by [@xusd320](https://github.com/xusd320) in [#1450](https://github.com/umijs/mako/pull/1450)
- feat: support sass by [@xiaohuoni] in [#1443](https://github.com/umijs/mako/pull/1443)
- feat: sass option support function by [@xiaohuoni](https://github.com/xiaohuoni) in [#1461](https://github.com/umijs/mako/pull/1461)
- fix: double value lose by [@xiaohuoni](https://github.com/xiaohuoni) in [#1462](https://github.com/umijs/mako/pull/1462)
- perf: use hashlink, speed up codeSplitting by 300% when building big project by [@xusd320](https://github.com/xusd320) in [#1460](https://github.com/umijs/mako/pull/1460)
- perf(tree-shaking): parallelize tree shaking module map init by [@stormslowly](https://github.com/stormslowly) in [#1452](https://github.com/umijs/mako/pull/1452)

## 0.7.8

`2024-07-25`

- feat(px2rem): add mediaQuery config by [@stormslowly](https://github.com/stormslowly) in [#1431](https://github.com/umijs/mako/pull/1431)
- feat: support \_\_webpack\_public\_path and \_\_mako\_public\_path assignment by [@sorrycc](https://github.com/sorrycc) in [#1441](https://github.com/umijs/mako/pull/1441)
- feat: sort stat by size in desc order by [@jason89521](https://github.com/jason89521) in [#1393](https://github.com/umijs/mako/pull/1393)
- fix: async module missing async deps after update by [@stormslowly](https://github.com/stormslowly) in [#1437](https://github.com/umijs/mako/pull/1437)
- fix: chunk file name should be url-friendly by [@PeachScript](https://github.com/PeachScript) in [#1434](https://github.com/umijs/mako/pull/1434)


## 0.7.7

`2024-07-23`

- Perf: remove an ast clone when tree-shaking by [@stormslowly](https://github.com/stormslowly) in [#1429](https://github.com/umijs/mako/pull/1429)
- Improvement: detect circular dependencies support ignore config by [@stormslowly](https://github.com/stormslowly) in [#1425](https://github.com/umijs/mako/pull/1425)
- Fix: not merge small async chunks to entry by [@xusd320](https://github.com/xusd320) in [#1397](https://github.com/umijs/mako/pull/1435)
- Fix: dev server support "publicPath" by [@whyer11](https://github.com/whyer11) and [@sorrycc](https://github.com/sorrycc) in [#1398](https://github.com/umijs/mako/pull/1398)
- Revert [#1385](https://github.com/umijs/mako/pull/1385) by [@Jinbao1001](https://github.com/Jinbao1001)


## 0.7.6

`2024-07-18`

- New: add umi template for `create-mako` by [@kiner-tang](https://github.com/kiner-tang) in [#1408](https://github.com/umijs/mako/pull/1408)
- New: circular dependency detector by [@stormslowly](https://github.com/stormslowly) in [#1401](https://github.com/umijs/mako/pull/1401)
- New: add `emitDecoratorMetadata` config by [@sorrycc](https://github.com/sorrycc) in [#1420](https://github.com/umijs/mako/pull/1420)
- New: support mako cli using abbreviated `mode` value, like "prod" by [@stormslowly](https://github.com/stormslowly) in [#1419](https://github.com/umijs/mako/pull/1419)
- Fix: config `mako.plugins` should work by [@sorrycc](https://github.com/sorrycc) in [#1400](https://github.com/umijs/mako/pull/1400)
- Fix: assignment failure when `plugins` are undefined by [@xierenyuan](https://github.com/xierenyuan) in [#1402](https://github.com/umijs/mako/pull/1402)
- Fix: support dynamic import with template string by [@sorrycc](https://github.com/sorrycc) in [#1405](https://github.com/umijs/mako/pull/1405)
- Fix: watch files change of module graph in node_modules by [@Jinbao1001](https://github.com/Jinbao1001) in [#1385](https://github.com/umijs/mako/pull/1385)
- Fix: dynamic import to require need interop by [@Jinbao1001](https://github.com/Jinbao1001) in [#1363](https://github.com/umijs/mako/pull/1361)

## 0.7.5

`2024-07-11`

- New: Added console warning for HMR if React is external by [@PeachScript](https://github.com/PeachScript) in [#1354](https://github.com/umijs/mako/pull/1354)
- New: CLI now supports custom project names by [@kiner-tang](https://github.com/kiner-tang) in [#1340](https://github.com/umijs/mako/pull/1340)
- New: Upgraded hyper-staticfile to fix JS file charset issues by [@whyer11](https://github.com/whyer11) in [#1363](https://github.com/umijs/mako/pull/1363)
- New: CLI now checks if there are existing files in the current directory by [@liangchaofei](https://github.com/liangchaofei) in [#1368](https://github.com/umijs/mako/pull/1368)
- New: Support for selecting templates from the templates directory by [@kiner-tang](https://github.com/kiner-tang) in [#1370](https://github.com/umijs/mako/pull/1370)
- New: px2rem now supports selectorDoubleRemList by [@xiaohuoni](https://github.com/xiaohuoni) in [#1336](https://github.com/umijs/mako/pull/1336)
- New: Pass umi configuration to mako by [@xiaohuoni](https://github.com/xiaohuoni) in [#1394](https://github.com/umijs/mako/pull/1394)
- Improvement: More idiomatic and concise SWC AST generation by [@stormslowly](https://github.com/stormslowly) in [#1372](https://github.com/umijs/mako/pull/1372)
- Improvement: Clearer code logic and types by [@xusd320](https://github.com/xusd320) in [#1397](https://github.com/umijs/mako/pull/1397)
- Fix: Decoded paths for less plugin by [@stormslowly](https://github.com/stormslowly) in [#1360](https://github.com/umijs/mako/pull/1360)
- Fix: Stringifying object values causing panic by [@xusd320](https://github.com/xusd320) in [#1349](https://github.com/umijs/mako/pull/1349)
- Fix: HMR does not support React.lazy + import() components by [@sorrycc](https://github.com/sorrycc) in [#1369](https://github.com/umijs/mako/pull/1369)
- Fix: Corrected spelling mistakes by [@kiner-tang](https://github.com/kiner-tang) in [#1371](https://github.com/umijs/mako/pull/1371)
- Fix: pnpm installation issues by [@sorrycc](https://github.com/sorrycc) in [#1376](https://github.com/umijs/mako/pull/1376)
- Fix: Unstable entry hash by [@stormslowly](https://github.com/stormslowly) in [#1374](https://github.com/umijs/mako/pull/1374)
- Fix: analyze not working in umi by [@sorrycc](https://github.com/sorrycc) in [#1387](https://github.com/umijs/mako/pull/1387)
- Fix: Loss of CSS order after sorting dependencies alphabetically by [@xusd320](https://github.com/xusd320) in [#1391](https://github.com/umijs/mako/pull/1391)
- Fix: Should check reserved words after preset_env by [@Jinbao1001](https://github.com/Jinbao1001) in [#1367](https://github.com/umijs/mako/pull/1367)
- Fix: commonjs might lack use strict directive by [@Jinbao1001](https://github.com/Jinbao1001) in [#1386](https://github.com/umijs/mako/pull/1386)


## 0.7.4

`2024-07-02`

- Fix code splitting granular strategy by [@xusd320](https://github.com/xusd320) in [#1318](https://github.com/umijs/mako/pull/1318)
- Fix part of the IDE errors reported by create-mako by [@programmer-yang](https://github.com/programmer-yang) in [#1345](https://github.com/umijs/mako/pull/1345)
- Fix create-mako stylesheet not hot-reloading by [@sorrycc](https://github.com/sorrycc) in [#1348](https://github.com/umijs/mako/pull/1348)
- Fix unnecessary clone in stats by [@xusd320](https://github.com/xusd320) in [#1351](https://github.com/umijs/mako/pull/1351)
- Fix undetected nested function expressions in concatenateModules by [@stormslowly](https://github.com/stormslowly) in [#1357](https://github.com/umijs/mako/pull/1357)
- Adjust file size unit symbols by [@hualigushi](https://github.com/hualigushi) in [#1320](https://github.com/umijs/mako/pull/1320)
- Documentation adjustments by [@kiner-tang](https://github.com/kiner-tang) in [#1337](https://github.com/umijs/mako/pull/1337) [#1339](https://github.com/umijs/mako/pull/1339)

## 0.7.3

`2024-07-01`

- Fix: Dynamic import of async modules by [@stormslowly](https://github.com/stormslowly) in [#1316](https://github.com/umijs/mako/pull/1316)
- Fix: Use vec instead of hash_map in alias by [@Jinbao1001](https://github.com/Jinbao1001) in [#1299](https://github.com/umijs/mako/pull/1299)
- Fix: Variable linking identifier conflicts with local variables by [@stormslowly](https://github.com/stormslowly) in [#1315](https://github.com/umijs/mako/pull/1315)
- Fix: Simplification by swc causing this to be undefined by [@Jinbao1001](https://github.com/Jinbao1001) in [#1294](https://github.com/umijs/mako/pull/1294)
- Fix: Code splitting mode automation may miss the connection between chunk and urlMap by [@Jinbao1001](https://github.com/Jinbao1001) in [#1311](https://github.com/umijs/mako/pull/1311)
- Other: Rename tree shaking by [@stormslowly](https://github.com/stormslowly) in [#1308](https://github.com/umijs/mako/pull/1308)
- Other: Add minifish ignore instructions by [@stormslowly](https://github.com/stormslowly) in [#1310](https://github.com/umijs/mako/pull/1310)

## 0.7.2

`2024-06-26`

- Improved module concatenate implementation, merged modules still support Shared Reference by [@stormslowly](https://github.com/stormslowly) in [#1295](https://github.com/umijs/mako/pull/1295)
- Fix hmr http response without setting content-type causing garbled text issue by [@whyer11](https://github.com/whyer11) in [#1307](https://github.com/umijs/mako/pull/1307)

## 0.7.1

`2024-06-20`

- Rollback "change alias from map to vec" by [@stormslowly](https://github.com/stormslowly) in [#1297](https://github.com/umijs/mako/pull/1297)

## 0.7.0

`2024-06-20`

- Added Code splitting granular strategy (breaking upgrade), enabled by GRANULAR\_CHUNKS environment variable by [@xusd320](https://github.com/xusd320) in [#1269](https://github.com/umijs/mako/pull/1269)
- Added using path configuration for plugins feature by [@sorrycc](https://github.com/sorrycc) in [#1292](https://github.com/umijs/mako/pull/1292)
- Added concurrent processing less files under node.js version 16 feature by [@xusd320](https://github.com/xusd320) in [#1280](https://github.com/umijs/mako/pull/1280)
- Improved alias configuration to vec to avoid unordered issue by [@Jinbao1001](https://github.com/Jinbao1001) in [#1289](https://github.com/umijs/mako/pull/1289)
- Fixed Symbol being overwritten by user code, causing lower version products unusable issue by [@stormslowly](https://github.com/stormslowly) in [#1279](https://github.com/umijs/mako/pull/1279)
- Fixed modules not going through Interop processing in dynamic import issue by [@stormslowly](https://github.com/stormslowly) in [#1209](https://github.com/umijs/mako/pull/1209)
- Fixed an issue where a module was unusable when simultaneously imported and referenced by a worker by [@xusd320](https://github.com/xusd320) in [#1278](https://github.com/umijs/mako/pull/1278)
- Fixed circular reference in export * issue by [@stormslowly](https://github.com/stormslowly) in [#1277](https://github.com/umijs/mako/pull/1277)
- Fixed the issue of the React variable being deleted in TypeScript using react classic mode by [@Jinbao1001](https://github.com/Jinbao1001) in [#1285](https://github.com/umijs/mako/pull/1285)
- Fixed wrong use of external configuration like `window.xxx` by [@xusd320](https://github.com/xusd320) in [#1293](https://github.com/umijs/mako/pull/1293)
- Fixed dynamic import unable to resolve part of the path in template strings issue by [@Jinbao1001](https://github.com/Jinbao1001) in [#1224](https://github.com/umijs/mako/pull/1224)

## 0.6.0

`2024-06-13`

- Added: Improved build API (includes Break Change) by [@sorrycc](https://github.com/sorrycc) in [#1271](https://github.com/umijs/mako/pull/1271)
- Added: Support for resource output with new URL() by [@sorrycc](https://github.com/sorrycc) in [#1261](https://github.com/umijs/mako/pull/1261)
- Added: Notify users that the current platform is not supported on win32 platform by [@sorrycc](https://github.com/sorrycc) in [#1262](https://github.com/umijs/mako/pull/1262)
- Added: Automatically find an available port when the port is occupied by [@sorrycc](https://github.com/sorrycc) in [#1266](https://github.com/umijs/mako/pull/1266)
- Added: Automatically open the browser when the development server is ready by [@sorrycc](https://github.com/sorrycc) in [#1267](https://github.com/umijs/mako/pull/1267)
- Added: Basic product analyze capability by [@LovePlayCode](https://github.com/LovePlayCode) in [#1228](https://github.com/umijs/mako/pull/1228)
- Fixed: Use non-blocking IO due to lack of support in Rust by [@xusd320](https://github.com/xusd320) in [#1252](https://github.com/umijs/mako/pull/1252)
- Fixed: globalThis property access by [@xusd320](https://github.com/xusd320) in [#1254](https://github.com/umijs/mako/pull/1254)
- Fixed: Problem of default export being skipped by [@stormslowly](https://github.com/stormslowly) in [#1257](https://github.com/umijs/mako/pull/1257)
- Fixed(concatenate): Export conflicts between root and internals by [@stormslowly](https://github.com/stormslowly) in [#1256](https://github.com/umijs/mako/pull/1256)
- Fixed(concatenate): Runtime execution order by [@stormslowly](https://github.com/stormslowly) in [#1263](https://github.com/umijs/mako/pull/1263)
- Fixed: try resolve should support config.ignores by [@sorrycc](https://github.com/sorrycc) in [#1264](https://github.com/umijs/mako/pull/1264)

## 0.5.4

`2024-06-06`

- Optimized: HMR optimization for runtime errors by [@sorrycc](https://github.com/sorrycc) in [#1244](https://github.com/umijs/mako/pull/1244)
- Fixed: dts mismatch by [@sorrycc](https://github.com/sorrycc) in [#1237](https://github.com/umijs/mako/pull/1237)
- Fixed: Re-export in the root directory by [@stormslowly](https://github.com/stormslowly) in [#1232](https://github.com/umijs/mako/pull/1232)
- Fixed: Worker circular dependency issue by [@xusd320](https://github.com/xusd320) in [#1247](https://github.com/umijs/mako/pull/1247)

## 0.5.3

`2024-06-04`

- Fixed: Update chunk URL mapping when adding async imports in watch mode by [@xusd320](https://github.com/xusd320) in [#1220](https://github.com/umijs/mako/pull/1220)
- Fixed: Pattern not starting with a dot not matched by [@stormslowly](https://github.com/stormslowly) in [#1230](https://github.com/umijs/mako/pull/1230)
- Fixed(fix_helper_inject_position): Missing export variable ctxt by [@sorrycc](https://github.com/sorrycc) in [#1236](https://github.com/umijs/mako/pull/1236)
- Optimization: Update mako bundler to accommodate new mako version by [@Jinbao1001](https://github.com/Jinbao1001) in [#1229](https://github.com/umijs/mako/pull/1229)

## 0.5.2

`2024-05-31`

- Add (experimental): SSU feature provided by [@stormslowly](https://github.com/stormslowly) in [#1186](https://github.com/umijs/mako/pull/1186)
- Fix: Do not generate hmr chunk and json when hmr is false by [@sorrycc](https://github.com/sorrycc) in [#1223](https://github.com/umijs/mako/pull/1223)
- Fix: Chunk runtime template incompatible with older devices by [@PeachScript](https://github.com/PeachScript) in [#1227](https://github.com/umijs/mako/pull/1227)
- Misc: Support local releases using musl by [@sorrycc](https://github.com/sorrycc) in [#1221](https://github.com/umijs/mako/pull/1221)

## 0.5.1

`2024-05-30`

- Add plugin-based extension of mako features by [@sorrycc](https://github.com/sorrycc) in [#1219](https://github.com/umijs/mako/pull/1219)
- Add support for x86_64 linux musl by [@stormslowly](https://github.com/stormslowly) in [#1218](https://github.com/umijs/mako/pull/1218)
- Fix module merge to correctly resolve module export symbols by [@stormslowly](https://github.com/stormslowly) in [#1216](https://github.com/umijs/mako/pull/1216)
- Fix the issue causing HRM errors under cyclic dependencies by [@stormslowly](https://github.com/stormslowly) in [#1191](https://github.com/umijs/mako/pull/1191)

## 0.5.0

`2024-05-29`

* Add watch.ignorePaths configuration by [@sorrycc](https://github.com/sorrycc) in [#1179](https://github.com/umijs/mako/pull/1179)
* Add support for externals and commonjs require by [@sorrycc](https://github.com/sorrycc) in [#1185](https://github.com/umijs/mako/pull/1185)
* Add rscClient.logServerComponent configuration by [@sorrycc](https://github.com/sorrycc) in [#1200](https://github.com/umijs/mako/pull/1200)
* Add stats.modules configuration to generate modules with dependencies and dependents by [@sorrycc](https://github.com/sorrycc) in [#1202](https://github.com/umijs/mako/pull/1202)
* Add useDefineForClassFields configuration by [@stormslowly](https://github.com/stormslowly) in [#1181](https://github.com/umijs/mako/pull/1181)
* Optimize watch, dev_server, and hmr configurations (includes Break Change) by [@sorrycc](https://github.com/sorrycc) in [#1206](https://github.com/umijs/mako/pull/1206)
* Optimize improvements in parseServerStats by [@sorrycc](https://github.com/sorrycc) in [#1203](https://github.com/umijs/mako/pull/1203)
* Fix hooks transmission loss issue by [@Jinbao1001](https://github.com/Jinbao1001) in [#1170](https://github.com/umijs/mako/pull/1170)
* Fix the "too many files open" error in the with-antd example during watch by [@zhangpanweb](https://github.com/zhangpanweb) in [#1022](https://github.com/umijs/mako/pull/1022)
* Fix decorator visitor should run before preset env by [@stormslowly](https://github.com/stormslowly) in [#1176](https://github.com/umijs/mako/pull/1176)
* Fix node scenario, add packages to be ignored by [@sorrycc](https://github.com/sorrycc) in [#1182](https://github.com/umijs/mako/pull/1182)
* Fix less, disable parallel less loader on Linux for node version < 20.12.0 by [@xusd320](https://github.com/xusd320) in [#1184](https://github.com/umijs/mako/pull/1184)
* Fix node version check in less loader by [@xusd320](https://github.com/xusd320) in [#1188](https://github.com/umijs/mako/pull/1188)
* Fix re-parser to add ctxt by [@stormslowly](https://github.com/stormslowly) in [#1189](https://github.com/umijs/mako/pull/1189)
* Fix px2rem min_pixel_value should accept absolute value by [@sorrycc](https://github.com/sorrycc) in [#1192](https://github.com/umijs/mako/pull/1192)
* Fix swc bug in exporting functions with array parameters in chrome 50 by [@sorrycc](https://github.com/sorrycc) in [#1199](https://github.com/umijs/mako/pull/1199)
* Fix duplicate assets information in watch mode by [@xusd320](https://github.com/xusd320) in [#1194](https://github.com/umijs/mako/pull/1194)
* Fix incorrect ctx type by [@stormslowly](https://github.com/stormslowly) in [#1196](https://github.com/umijs/mako/pull/1196)
* Fix rsc support for moduleIdStrategy hashed by [@sorrycc](https://github.com/sorrycc) in [#1201](https://github.com/umijs/mako/pull/1201)
* Fix fix_helper_inject_position to support exported const arrow functions by [@sorrycc](https://github.com/sorrycc) in [#1207](https://github.com/umijs/mako/pull/1207)
* Fix stripping of exported namespace types in ts by [@stormslowly](https://github.com/stormslowly) in [#1198](https://github.com/umijs/mako/pull/1198)
* Fix panic on wrong watch result event by [@sorrycc](https://github.com/sorrycc) in [#1212](https://github.com/umijs/mako/pull/1212)
* Fix should regroup when adding dynamic dependencies in watch mode by [@xusd320](https://github.com/xusd320) in [#1213](https://github.com/umijs/mako/pull/1213)
* Fixed inlineCSS not working by [@stormslowly](https://github.com/stormslowly) in [#1211](https://github.com/umijs/mako/pull/1211)

## 0.4.17

`2024-05-16`

* Added watch=parent support by [@sorrycc](https://github.com/sorrycc) in [#1151](https://github.com/umijs/mako/pull/1151)
* Added create-mako package by [@sorrycc](https://github.com/sorrycc) in [#1164](https://github.com/umijs/mako/pull/1164)
* Added: Remove output.ascii_only configuration by [@sorrycc](https://github.com/sorrycc) in [#1152](https://github.com/umijs/mako/pull/1152)
* Optimized less, support for less plugins by [@xusd320](https://github.com/xusd320) in [#1148](https://github.com/umijs/mako/pull/1148)
* Optimized less, compatible with ESM less plugins by [@PeachScript](https://github.com/PeachScript) in [#1162](https://github.com/umijs/mako/pull/1162)
* Optimized stats.json, added modules property by [@sorrycc](https://github.com/sorrycc) in [#1167](https://github.com/umijs/mako/pull/1167)
* Fixed empty chunk issue by [@stormslowly](https://github.com/stormslowly) in [#1147](https://github.com/umijs/mako/pull/1147)
* Fixed ESM and require mixing issue by [@stormslowly](https://github.com/stormslowly) in [#1154](https://github.com/umijs/mako/pull/1154)
* Fixed panic issue when generating empty chunks by [@xusd320](https://github.com/xusd320) in [#1135](https://github.com/umijs/mako/pull/1135)
* Fixed tree-shaking imported modules not returning namespace issue by [@stormslowly](https://github.com/stormslowly) in [#1158](https://github.com/umijs/mako/pull/1158)
* Fixed retaining Chinese characters in bundless mode by [@sorrycc](https://github.com/sorrycc) in [#1160](https://github.com/umijs/mako/pull/1160)
* Fixed incorrect chunk size map issue by [@xusd320](https://github.com/xusd320) in [#1161](https://github.com/umijs/mako/pull/1161)
* Fixed missing sibling modules in client chunk in rsc sdk by [@PeachScript](https://github.com/PeachScript) in [#1166](https://github.com/umijs/mako/pull/1166)

## 0.4.16

`2024-05-11`

* Fixed Chinese characters in artifacts not converted to unicode issue by [@sorrycc](https://github.com/sorrycc) in [#1146](https://github.com/umijs/mako/pull/1146)
* Fixed the issue of merging ignored modules during module merge optimization causing undefined variables by [@stormslowly](https://github.com/stormslowly) in [#1149](https://github.com/umijs/mako/pull/1149)

## 0.4.15

`2024-05-10`

* Optimize px2rem support for min_pixel_value configuration by [@sorrycc](https://github.com/sorrycc) in [#1141](https://github.com/umijs/mako/pull/1141)
* Fix the issue where px2rem would panic when using attribute selectors without a value by [@sorrycc](https://github.com/sorrycc) in [#1140](https://github.com/umijs/mako/pull/1140)
* Fix the issue that the node patch solution does not support timers by [@sorrycc](https://github.com/sorrycc) in [#1142](https://github.com/umijs/mako/pull/1142)

## 0.4.14

`2024-05-09`

* Turn on concatenate modules by default by [@stormslowly](https://github.com/stormslowly) in [#1126](https://github.com/umijs/mako/pull/1126)
* Fix the potential instability of chunk id ordering by [@stormslowly](https://github.com/stormslowly) in [#1117](https://github.com/umijs/mako/pull/1117)
* chore: add log for parallel generate by [@xusd320](https://github.com/xusd320) in [#1127](https://github.com/umijs/mako/pull/1127)
* Fix the issue where re-grouping of chunk does not happen when dependency types change in a hot update scenario by [@xusd320](https://github.com/xusd320) in [#1124](https://github.com/umijs/mako/pull/1124)

## 0.4.13

`2024-05-06`

* Add support for specifying the path of a virtual file through ?path by [@stormslowly](https://github.com/stormslowly) in [#1102](https://github.com/umijs/mako/pull/1102)
* Add global `__mako_chunk_load__` method by [@sorrycc](https://github.com/sorrycc) in [#1111](https://github.com/umijs/mako/pull/1111)
* Optimize mako CLI to support specifying mode by [@sorrycc](https://github.com/sorrycc) in [#1114](https://github.com/umijs/mako/pull/1114)
* Fix concatenate inner global var conflict with other modules' top level vars by [@stormslowly](https://github.com/stormslowly) in [#1100](https://github.com/umijs/mako/pull/1100)
* Fix the issue that node polyfill does not work in identifier shorthand scenarios by [@stormslowly](https://github.com/stormslowly) in [#1104](https://github.com/umijs/mako/pull/1104)
* Fix the issue where manifest is not output during the dev stage by [@sorrycc](https://github.com/sorrycc) in [#1106](https://github.com/umijs/mako/pull/1106)
* Fix the issue where stats.json is not output during the dev stage by [@sorrycc](https://github.com/sorrycc) in [#1108](https://github.com/umijs/mako/pull/1108)
* Fix the cjs build scenario (for SSR) by [@Jinbao1001](https://github.com/Jinbao1001) in [#1109](https://github.com/umijs/mako/pull/1109)
* Refactor to remove lazy_static by [@xusd320](https://github.com/xusd320) in [#1103](https://github.com/umijs/mako/pull/1103)
* Refactor the overall directory structure by [@sorrycc](https://github.com/sorrycc) in [#1105](https://github.com/umijs/mako/pull/1105)
* Refactor okam to mako, while also making the @alipay scope's packages public under @umijs by [@sorrycc](https://github.com/sorrycc) in [#1113](https://github.com/umijs/mako/pull/1113)

## 0.4.12

`2024-04-28`

* Fix the issue where the bin field in the okam package's package.json was missing by [@sorrycc](https://github.com/sorrycc) in [#1092](https://github.com/umijs/mako/pull/1092)
* Fix runtime error in node environments by ensuring css is only loaded during the browser phase by [@sorrycc](https://github.com/sorrycc) in [#1095](https://github.com/umijs/mako/pull/1095)
* Fix the issue where empty css chunks should not be output by [@xusd320](https://github.com/xusd320) in [#1097](https://github.com/umijs/mako/pull/1097)
* Fix the issue where css should not be loaded in node environments (potential performance improvement) by [@sorrycc](https://github.com/sorrycc) in [#1098](https://github.com/umijs/mako/pull/1098)
* Fix the issue where polyfills were not replaced within inner in concatenate by [@stormslowly](https://github.com/stormslowly) in [#1099](https://github.com/umijs/mako/pull/1099)

## 0.4.11

`2024-04-25`

* Add RSC functionality by [@sorrycc](https://github.com/sorrycc) in [#1063](https://github.com/umijs/mako/pull/1063)
* Add RSC sdk by [@sorrycc](https://github.com/sorrycc) in [#1072](https://github.com/umijs/mako/pull/1072)
* Add loader return parameter increase jsx property by [@sorrycc](https://github.com/sorrycc) in [#1079](https://github.com/umijs/mako/pull/1079)
* Add experimental.webpackSyntaxValidate configuration by [@sorrycc](https://github.com/sorrycc) in [#1080](https://github.com/umijs/mako/pull/1080)
* Add okam cli by [@sorrycc](https://github.com/sorrycc) in [#1087](https://github.com/umijs/mako/pull/1087)
* Add support for css_rem attribute selector by [@LovePlayCode](https://github.com/LovePlayCode) in [#1059](https://github.com/umijs/mako/pull/1059)
* Add support for pseudo-class selectors by [@LovePlayCode](https://github.com/LovePlayCode) in [#1061](https://github.com/umijs/mako/pull/1061)
* Fix okam TS type issue BuildParams by [@sorrycc](https://github.com/sorrycc) in [#1073](https://github.com/umijs/mako/pull/1073)
* Fix global variable access at runtime with mako by [@PeachScript](https://github.com/PeachScript) in [#1082](https://github.com/umijs/mako/pull/1082)
* Fix unstable css order by [@xusd320](https://github.com/xusd320) in [#1085](https://github.com/umijs/mako/pull/1085)

## 0.4.10

`2024-04-16`

* Add support for forkTSChecker by [@ctts](https://github.com/ctts) and @sorrycc in [#956](https://github.com/umijs/mako/pull/956)
* Optimize generate to parallelize entry execution, speeding up by 10% by [@xusd320](https://github.com/xusd320) in [#1001](https://github.com/umijs/mako/pull/1001)
* Optimize px2rem support for selector_black_list and selector_white_list by [@LovePlayCode](https://github.com/LovePlayCode) and @sorrycc in [#1043](https://github.com/umijs/mako/pull/1043)
* Enhance less loader implementation based on worker, increasing performance by 20% by [@xusd320](https://github.com/xusd320) in [#1048](https://github.com/umijs/mako/pull/1048)
* Optimize importInfo, delete unused specifier by [@goo-yyh](https://github.com/goo-yyh) in [#963](https://github.com/umijs/mako/pull/963)
* Optimize sourcemap file path, moving internal runtime code to mako_internal directory by [@stormslowly](https://github.com/stormslowly) in [#1055](https://github.com/umijs/mako/pull/1055)
* Optimize ast to code performance, execute concurrently in dev by [@xusd320](https://github.com/xusd320) in [#1053](https://github.com/umijs/mako/pull/1053)
* Refactor packages/mako into an entry package by [@sorrycc](https://github.com/sorrycc) in [#1010](https://github.com/umijs/mako/pull/1010)
* Refactor the implementation of @okamjs/okam, encapsulate less and other features by [@sorrycc](https://github.com/sorrycc) in [#1024](https://github.com/umijs/mako/pull/1024)
* Fix the implementation of concatenateModules, var ident conflict with root's top vars by [@stormslowly](https://github.com/stormslowly) in [#1052](https://github.com/umijs/mako/pull/1052)
* Fix the issue that dynamic_import_to_require must be executed after context_require by [@sorrycc](https://github.com/sorrycc) in [#1038](https://github.com/umijs/mako/pull/1038)
* Fix tree shaking support for multiple declarator declares by [@stormslowly](https://github.com/stormslowly) in [#1032](https://github.com/umijs/mako/pull/1032)
* Fix provider, change unresolved indent syntax context to top level after it's been declared by [@stormslowly](https://github.com/stormslowly) in [#1027](https://github.com/umijs/mako/pull/1027)
* Fix `unwrap()` panic in update phase by [@sorrycc](https://github.com/sorrycc) in [#1004](https://github.com/umijs/mako/pull/1004)
* Fix `concatenateModule`, treat module as external when it contains unsupported syntax by [@stormslowly](https://github.com/stormslowly) in [#1009](https://github.com/umijs/mako/pull/1009)

## 0.4.9

`2024-04-01`

* Fix issue where isolated chunks appear in chunk optimization by [@Jinbao1001](https://github.com/Jinbao1001) in [#988](https://github.com/umijs/mako/pull/988)
* Fix unstable entry chunk hash issue by [@xusd320](https://github.com/xusd320) in [#1003](https://github.com/umijs/mako/pull/1003)
* Fix `concatenateModules` unable to merge multiple external modules issue [@stormslowly](https://github.com/stormslowly) in [#1005](https://github.com/umijs/mako/pull/1005)

## 0.4.8

`2024-03-23`

* Add scope hoist feature, configurable by [@stormslowly](https://github.com/stormslowly) in [#922](https://github.com/umijs/mako/pull/922)
* Fix js hook should use full path issue by [@Jinbao1001](https://github.com/Jinbao1001) in [#987](https://github.com/umijs/mako/pull/987)
* Reduce performance overhead during tree shaking phase by [@xusd320](https://github.com/xusd320) in [#980](https://github.com/umijs/mako/pull/980)
* Remove regex in node_polyfill to improve performance by [@sorrycc](https://github.com/sorrycc) in [#998](https://github.com/umijs/mako/pull/998)
* Refactor generate cache hash handling by [@xusd320](https://github.com/xusd320) in [#992](https://github.com/umijs/mako/pull/992)

## 0.4.7

`2024-03-22`

* Fix boundary scenario of fast refresh generating components inside functions by [@sorrycc](https://github.com/sorrycc) in [#972](https://github.com/umijs/mako/pull/972)
* Fix scenario when referencing assets with query by [@sorrycc](https://github.com/sorrycc) in [#975](https://github.com/umijs/mako/pull/975)

## 0.4.6

`2024-03-20`

* Fix resolve fragment issue, support scenario of a#b.ts by [@sorrycc](https://github.com/sorrycc) in [#966](https://github.com/umijs/mako/pull/966)

## 0.4.5

`2024-03-20`

* Refactor part of the build code by [@sorrycc](https://github.com/sorrycc) in [#923](https://github.com/umijs/mako/pull/923)
* Add HMR Fast Refresh support for anonymous functions scenario by [@JackGuiYang12](https://github.com/JackGuiYang12) in [#947](https://github.com/umijs/mako/pull/947)
* Add inline_css configuration, implement style-loader like functionality by [@sorrycc](https://github.com/sorrycc) in [#957](https://github.com/umijs/mako/pull/957)
* Optimize the use of rayon, allowing generate to reuse build stage's rayon threads by [@xusd320](https://github.com/xusd320) in [#959](https://github.com/umijs/mako/pull/959)
* Enhance minifish inject feature, support include configuration item by [@stormslowly](https://github.com/stormslowly) in [#930](https://github.com/umijs/mako/pull/930)
* Fix async chunk should not split root module by [@PeachScript](https://github.com/PeachScript) in [#929](https://github.com/umijs/mako/pull/929)
* Fix css url() should support # prefix by [@sorrycc](https://github.com/sorrycc) in [#949](https://github.com/umijs/mako/pull/949)
* Fix the implementation of async module by [@stormslowly](https://github.com/stormslowly) in [#943](https://github.com/umijs/mako/pull/943)
* Fix the support for # fragment when resolving js and css dependencies by [@sorrycc](https://github.com/sorrycc) in [#952](https://github.com/umijs/mako/pull/952)
* Fix support for non-ascii paths, such as spaces and Chinese characters by [@sorrycc](https://github.com/sorrycc) in [#958](https://github.com/umijs/mako/pull/958)
* Fix ignored modules should be compiled into empty es modules by [@xusd320](https://github.com/xusd320) in [#946](https://github.com/umijs/mako/pull/946)
* Fix in context module scenarios, async import should be split by [@xusd320](https://github.com/xusd320) in [#940](https://github.com/umijs/mako/pull/940)
* Fix the stats information of sync chunk by [@PeachScript](https://github.com/PeachScript) in [#928](https://github.com/umijs/mako/pull/928)

## 0.4.4

`2024-02-29`

- Fix issues where dynamic require/import in call_expr were not being correctly transformed by [@PeachScript](https://github.com/PeachScript) in [#898](https://github.com/umijs/mako/pull/898)
- Compatibility with extraBabelPlugins: ['@emotion'] plugin configuration by [@sorrycc](https://github.com/sorrycc) in [#908](https://github.com/umijs/mako/pull/908)
- Use more efficient memory allocators (mimalloc-rust, tikv-jemallocator), M1 Pro yuyanAssets build see a stable improvement of approximately 2500ms by [@xusd320](https://github.com/xusd320) in [#912](https://github.com/umijs/mako/pull/912)
- Optimize the instantiation overhead of regular expressions in external features, M1 Pro yuyanAssets build see a stable improvement of approximately 3900ms by [@PeachScript](https://github.com/PeachScript) in [#916](https://github.com/umijs/mako/pull/916)
- Pass the full stats compilation data when calling the onBuildComplete hook by [@PeachScript](https://github.com/PeachScript) in [#917](https://github.com/umijs/mako/pull/917)
- Switch from nodejs-resolver to oxc_resolver by [@xusd320](https://github.com/xusd320) in [#919](https://github.com/umijs/mako/pull/919)

## 0.4.3

`2024-02-01`

- Fix the issue where skipModules misidentifies the export source in edge cases by [@stormslowly](https://github.com/stormslowly) in [#906](https://github.com/umijs/mako/pull/906)
- Roll back the SWC upgrade PR [#876](https://github.com/umijs/mako/pull/876) by [@stormslowly](https://github.com/stormslowly) in [#907](https://github.com/umijs/mako/pull/907)

## 0.4.2

`2024-01-31`

- Fix the issue where lessLoader.modifyVars does not take effect in dev environment by [@sorrycc](https://github.com/sorrycc) in [#900](https://github.com/umijs/mako/pull/900)
- Fix the OS error 35 caused by node binding due to mismatched stout/stderr modes by [@sorrycc](https://github.com/sorrycc) in [#901](https://github.com/umijs/mako/pull/901)
- Fix the bug where sideEffects configuration as relative paths in package.json led to incorrect sideEffects matching by [@stormslowly](https://github.com/stormslowly) in [#902](https://github.com/umijs/mako/pull/902)

## 0.4.1

`2024-01-30`

* Add HMR support for debugging npm packages linked by [@zhangpanweb](https://github.com/zhangpanweb) in [#864](https://github.com/umijs/mako/pull/864)
* Add support similar to raw-loader, enabled by adding a ?raw query by [@ctts](https://github.com/ctts) in [#877](https://github.com/umijs/mako/pull/877)
* Add cjs output configuration by [@sorrycc](https://github.com/sorrycc) in [#886](https://github.com/umijs/mako/pull/886)
* Add preload support for async script by [@PeachScript](https://github.com/PeachScript) in [#895](https://github.com/umijs/mako/pull/895)
* Add emit_assets and css_modules_export_only_locales configuration by [@sorrycc](https://github.com/sorrycc) in [#890](https://github.com/umijs/mako/pull/890)
* Upgrade swc to 86 by [@goo-yyh](https://github.com/goo-yyh) in [#876](https://github.com/umijs/mako/pull/876)
* Improve support for __dirname and __filename in node scenarios by [@zhangpanweb](https://github.com/zhangpanweb) in [#885](https://github.com/umijs/mako/pull/885)
* Optimize code splitting support in platform: node scenarios by [@sorrycc](https://github.com/sorrycc) in [#887](https://github.com/umijs/mako/pull/887)
* Optimize the method of checking if variables are declared to improve speed by [@zhangpanweb](https://github.com/zhangpanweb) in [#897](https://github.com/umijs/mako/pull/897)
* Optimize stats information, add siblings and origins information by [@PeachScript](https://github.com/PeachScript) in [#893](https://github.com/umijs/mako/pull/893)
* Refactor the implementation of the emotion plugin by [@zhangpanweb](https://github.com/zhangpanweb) in [#884](https://github.com/umijs/mako/pull/884)

## 0.4.0

`2024-01-18`

* Add new react configuration options, supporting different react runtime parameters by [@sorrycc](https://github.com/sorrycc) in [#872](https://github.com/umijs/mako/pull/872)
* Add friendly prompts when there are errors in mako.config.json by [@sorrycc](https://github.com/sorrycc) in [#875](https://github.com/umijs/mako/pull/875)
* Fix issue where HMR could not recover from file errors by [@sorrycc](https://github.com/sorrycc) in [#863](https://github.com/umijs/mako/pull/863)
* Fix Less parameter value reading priority, first modifyVars field, then theme by [@sorrycc](https://github.com/sorrycc) in [#874](https://github.com/umijs/mako/pull/874)
* Fix issue with style file import statements not being deleted by [@stormslowly](https://github.com/stormslowly) in [#869](https://github.com/umijs/mako/pull/869)
* Fix the side effect of losing default import when optimizing skipModule [@stormslowly](https://github.com/stormslowly) in [#862](https://github.com/umijs/mako/pull/862)

## 0.3.1

`2024-01-11`

* Fix the issue of product module ID annotations being unexpectedly cut off by glob expressions, causing runtime errors by [@PeachScript](https://github.com/PeachScript) in [#856](https://github.com/umijs/mako/pull/856)
* Optimize built-in plugin hooks and exposed js hooks by [@sorrycc](https://github.com/sorrycc) in [#855](https://github.com/umijs/mako/pull/855)

## 0.2.3

`2024-01-10`

* Temporarily disable skipModules optimization by [@stormslowly](https://github.com/stormslowly) in [#854](https://github.com/umijs/mako/pull/854)

## 0.2.2

`2024-01-09`

* Fix the undefined variable problem caused by using `as` multiple times for the same export in import/export statements by [@stormslowly](https://github.com/stormslowly) in [#850](https://github.com/umijs/mako/pull/850)
* Fix the issue where missing dependencies on dev start up still cause build failure after being complemented by [@zhangpanweb](https://github.com/zhangpanweb) in [#845](https://github.com/umijs/mako/pull/845)
* Fix the potential failure of parsing imported css in less files via relative paths by [@sorrycc](https://github.com/sorrycc) in [#844](https://github.com/umijs/mako/pull/844)
* Optimize artifact generation, keep comments when not compressing for easier debugging by [@sorrycc](https://github.com/sorrycc) in [#848](https://github.com/umijs/mako/pull/848)
* Optimize artifact generation, add ID comments to module declarations for easier debugging by [@sorrycc](https://github.com/sorrycc) in [#849](https://github.com/umijs/mako/pull/849)

## 0.2.1

`2024-01-04`

* Fix issue where dependencies were mistakenly deleted when `skipModules` optimization was used with both import and require on the same module by [@stormslowly](https://github.com/stormslowly) in [#842](https://github.com/umijs/mako/pull/842)

## 0.2.0

`2024-01-04`

* BREAK CHANGE: Adjust and merge configuration items by [@PeachScript](https://github.com/PeachScript) in [#837](https://github.com/umijs/mako/pull/837)
* Optimize `optimizePackageImports` feature, support caching, `export *`, and deep bucket files by [@sorrycc](https://github.com/sorrycc) in [#810](https://github.com/umijs/mako/pull/810)
* Optimize tree shaking, implement skip modules to further reduce artifact size by [@stormslowly](https://github.com/stormslowly) in [#807](https://github.com/umijs/mako/pull/807)
* Optimize stats, add entry information by [@PeachScript](https://github.com/PeachScript) in [#823](https://github.com/umijs/mako/pull/823) #829
* Fix a piece of ES5 incompatible code in runtime in [#830](https://github.com/umijs/mako/pull/830)
* Fix the issue of cjs modules being inserted with esm helpers turning them into esm modules by [@stormslowly](https://github.com/stormslowly) in [#831](https://github.com/umijs/mako/pull/831)
* Fix handling of default export in `optimizePackageImports` by [@zhangpanweb](https://github.com/zhangpanweb) in [#832](https://github.com/umijs/mako/pull/832)
* Optimize performance issue of skip modules by [@stormslowly](https://github.com/stormslowly) in [#826](https://github.com/umijs/mako/pull/826)
* Optimize a minor performance detail by [@stormslowly](https://github.com/stormslowly) in [#835](https://github.com/umijs/mako/pull/835)

## 0.1.15

`2023-12-18`

* Add support for displaying less code sourcemap by [@jiesia](https://github.com/jiesia) in [#755](https://github.com/umijs/mako/pull/775)
* Fix issue where require context doesn't support suffix require by [@PeachScript](https://github.com/PeachScript) in [#806](https://github.com/umijs/mako/pull/806)
* Fix duplicate modules issue in entry chunk and vendor chunk by [@PeachScript](https://github.com/PeachScript) in [#809](https://github.com/umijs/mako/pull/809)
* Fix the problem of missing dynamic reference modules after enabling dynamicImportToRequire by [@stormslowly](https://github.com/stormslowly) in [#811](https://github.com/umijs/mako/pull/811)

## 0.1.14

`2023-12-18`

* Optimize chunk cache to not use cache by default when building by [@zhangpanweb](https://github.com/zhangpanweb) in [#800](https://github.com/umijs/mako/pull/800)
* Fix the potential failure of loading chunks after the build caused by the hashed id strategy by [@PeachScript](https://github.com/PeachScript) in [#805](https://github.com/umijs/mako/pull/805)
* Fix error handling when require fails under try statement, and no error is reported on the command line in dev by [@sorrycc](https://github.com/sorrycc) in [#803](https://github.com/umijs/mako/pull/803)
* Fix that react refresh runtime code should not appear in HMR scenario with platform:node by [@sorrycc](https://github.com/sorrycc) in [#802](https://github.com/umijs/mako/pull/802)
* Fix the issue where the output is empty when devtool is none by [@zhangpanweb](https://github.com/zhangpanweb) in [#801](https://github.com/umijs/mako/pull/801)
* Fix the potential failure of loading non-entry chunks by [@PeachScript](https://github.com/PeachScript) in [#798](https://github.com/umijs/mako/pull/798)
* Refactor chunks collection algorithm to avoid potential stack overflow problems by [@PeachScript](https://github.com/PeachScript) in [#799](https://github.com/umijs/mako/pull/799)

## 0.1.12

`2023-12-14`

* Add support for shared chunk with multiple entries by [@PeachScript](https://github.com/PeachScript) in [#789](https://github.com/umijs/mako/pull/789)
* Fix the issue where SWC Helper fails to inject when module_id_strategy is hashed by [@sorrycc](https://github.com/sorrycc) in [#797](https://github.com/umijs/mako/pull/797)
* Fix the potential deadlock when optimizing chunk during HMR by [@PeachScript](https://github.com/PeachScript) in [#795](https://github.com/umijs/mako/pull/795)

## 0.1.11

`2023-12-14`

* Fix the issue where CSS hot update fails when runtimePublicPath is enabled and its final value includes origin by [@PeachScript](https://github.com/PeachScript) in [#768](https://github.com/umijs/mako/pull/768)
* Fix the compilation failure when requiring dynamic directories with pseudo suffixes by [@PeachScript](https://github.com/PeachScript) in [#778](https://github.com/umijs/mako/pull/778)
* Fix the potential loss of chunks when CSS file contents are identical by [@stormslowly](https://github.com/stormslowly) in [#781](https://github.com/umijs/mako/pull/781)

* Optimize the size of runtime, generate capabilities on demand by [@sorrycc](https://github.com/sorrycc) in [#767](https://github.com/umijs/mako/pull/767)
* Optimize chunk load and registration logic, support non-entry chunks loading before entry chunks by [@PeachScript](https://github.com/PeachScript) in [#783](https://github.com/umijs/mako/pull/783)

## 0.1.10

`2023-12-08`

* Adjust swc related dependencies to switch to swc_core by [@goo-yyh](https://github.com/goo-yyh) in [#765](https://github.com/umijs/mako/pull/765)
* Adjust tree-shaking to add judgment on side effects of variable declaration statements by [@stormslowly](https://github.com/stormslowly) in [#763](https://github.com/umijs/mako/pull/763)
* Fix node binding TypeScript definitions by [@stormslowly](https://github.com/stormslowly) in [#761](https://github.com/umijs/mako/pull/761)

## 0.1.9

`2023-12-07`

- Add support for dynamic strings in require, such as `require('./i18n' + lang)` by [@PeachScript](https://github.com/PeachScript) in [#747](https://github.com/umijs/mako/pull/747)
- Adjust tree-shaking, optimize handling of side effects by [@stormslowly](https://github.com/stormslowly) in [#725](https://github.com/umijs/mako/pull/725)
- Refactor watch, dev, and update logic, add debounce to support git checkout triggering multiple modifications at once by [@sorrycc](https://github.com/sorrycc) in [#744](https://github.com/umijs/mako/pull/744)
- Fix the issue where import() with empty content causes panic errors by [@sorrycc](https://github.com/sorrycc) in [#743](https://github.com/umijs/mako/pull/743)
- Fix the issue where require(css_file) is processed as css modules by [@sorrycc](https://github.com/sorrycc) in [#751](https://github.com/umijs/mako/pull/751)
- Fix the issue where the node patch scheme does not support class fs/promise references by [@sorrycc](https://github.com/sorrycc) in [#746](https://github.com/umijs/mako/pull/746)
- Fix the issue where dynamically loaded CSS via import() does not take effect by [@jiesia](https://github.com/jiesia) in [#756](https://github.com/umijs/mako/pull/756)
- Fix the issue where the worker doesn't support dynamically loading with import() by [@jiesia](https://github.com/jiesia) in [#755](https://github.com/umijs/mako/pull/755)
- Fix the occasional occurrence of process undefined during HMR by [@sorrycc](https://github.com/sorrycc) in [#741](https://github.com/umijs/mako/pull/741)
- Fix external configuration format judgment logic by [@PeachScript](https://github.com/PeachScript) in [#735](https://github.com/umijs/mako/pull/735)

- Fixed Minifish's inject function to support configuration of preferRequire and the order of inserted code based on the appearance order of the insertion variables by [@stormslowly](https://github.com/stormslowly) in [#731](https://github.com/umijs/mako/pull/731) #734

## 0.1.5

`2023-11-28`

- Added flexBugs configuration option, and it's enabled by default in the umi scenario by [@PeachScript](https://github.com/PeachScript) in [#728](https://github.com/umijs/mako/pull/728)
- Fixed the dts issue of the okam node package by [@stormslowly](https://github.com/stormslowly) in [#726](https://github.com/umijs/mako/pull/726)

## 0.1.3

`2023-11-27`

- Fixed an issue where merging into the common async chunk caused a data error in generating ensure statements, leading to loading failure by [@PeachScript](https://github.com/PeachScript) in [#712](https://github.com/umijs/mako/pull/712)
- Fixed the issue that require.loadScript was not replaced with `__mako__require__` by [@stormslowly](https://github.com/stormslowly) in [#715](https://github.com/umijs/mako/pull/715)
- Added dts for the node api by [@stormslowly](https://github.com/stormslowly) in [#716](https://github.com/umijs/mako/pull/716)

## 0.1.0

`2023-11-23`

- Added Emotion support by [@zhangpanweb](https://github.com/zhangpanweb) in [#694](https://github.com/umijs/mako/pull/694)
- Improved performance of generating chunks, stable improvements of about 200ms in m1 yuyanAssets build by [@sorrycc](https://github.com/sorrycc) in [#709](https://github.com/umijs/mako/pull/709)
- Improved performance of the generate phase transform, reducing the time cost from 3431ms to 1019ms in m1 yuyanAssets by [@sorrycc](https://github.com/sorrycc) in [#707](https://github.com/umijs/mako/pull/707)
- Fixed the issue that window.require should not be replaced with `window.__mako_require__` by [@jiesia](https://github.com/jiesia) in [#708](https://github.com/umijs/mako/pull/708)
- Fixed the issue of not handling errors in the child process when transforming_in_generate was multi-threaded by [@sorrycc](https://github.com/sorrycc) in [#710](https://github.com/umijs/mako/pull/710)
- Captured more unsupported loader syntaxes, such as file-loader?esModule=false!./src-noconflict/theme-kr_theme.js by [@sorrycc](https://github.com/sorrycc) in [#704](https://github.com/umijs/mako/pull/704)
