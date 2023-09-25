# Contributing

## Getting Started

Clone.

```bash
$ git clone git@github.com:umijs/mako.git
$ cd mako
```

Compile.

```bash
$ cargo build
$ cargo build --release
```

Run.

```bash
$ cargo run --bin mako examples/normal
# filter logs
$ RUST_LOG=mako=debug,info cargo run --bin mako examples/normal
$ RUST_LOG=mako::parse=debug,info cargo run --bin mako examples/normal
```

Run with HMR.

```bash
$ cargo run --bin mako examples/with-dynamic-import --watch
```

Dev.

```bash
$ pnpm dev examples/normal
```

Test.

```bash
$ cargo test
$ cargo insta review
```

Coverage.

```bash
$ cargo codecov
$ cargo codecov --html && open target/llvm-cov/html/index.html
```

Format.

```bash
$ cargo fmt
```

Lint.

```bash
$ cargo lint
```

Release.

```bash
$ npm run release
# After released successful, you need to release bundler-okam manually.
$ npm run release:bundler-okam
```

Release Dev Version.

Only *canary* and *dev* tags are allowed to be published to npm.

```bash
$ cd crates/node
$ pnpm esno scripts/release-dev.ts
```

[Ref](https://github.com/umijs/mako/pull/335)

Upgrade dependencies.

```bash
$ cargo upgrade
$ cargo upgrade --incompatible
$ cargo upgrade --dry-run
```

Benchmark with-antd example.

```bash
$ cargo build --release
$ time ./target/release/mako examples/with-antd
# or using hyperfine
$ hyperfine --runs 10 "./target/release/mako examples/with-antd"
```

Performance analysis with puffin.

```bash
$ cargo build --release
$ MAKO_PROFILE=1 ./target/release/mako examples/normal --mode=production
```

Use mako in umi or bigfish.

```bash
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js umi build --dev
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js max build --dev
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js bigfish build --dev
```

## Project Structure

There are 1 crate, 1 mixed (as crate and npm package at the same time) entity and 1 npm package in this project.

### `mako`

`mako` is the core crate, and the summary of directory structure is as follows:

```bash
crates/mako/src
├── analyze_deps.rs               # analyze deps from js/css ast
├── analyze_statement.rs          # analyze import/export statements from js ast, for tree-shaking
├── ast.rs                        # parse source to ast and parse ast to code and sourcemap
├── bfs.rs                        # util for breadth-first search
├── build.rs                      # transform source code to ast and combine into module graph
├── chunk.rs                      # structure to describe chunk
├── chunk_graph.rs                # structure to manage chunk graph
├── cli.rs                        # cli arguments parser
├── comments.rs                   # replace unused module and identifier with comments, for tree-shaking
├── compiler.rs                   # compile project according to user config
├── config.rs                     # serialize and watch user config
├── copy.rs                       # util for copy files to dist
├── defined_ident_collector.rs    # collect defined identifiers and check if it is used, for tree-shaking
├── dev.rs                        # serve project in watch mode
├── generate.rs                   # generate modules and assets to dist
├── generate_chunks.rs            # generate chunks from module graph and chunk graph
├── group_chunk.rs                # split module graph into chunks
├── hmr.rs                        # compile project for hmr
├── lib.rs                        # declare all mako modules
├── load.rs                       # load file content or base64 (assets only)
├── logger.rs                     # configure log error
├── main.rs                       # the entry of this crate
├── minify.rs                     # minify js code via ast
├── module.rs                     # structure to describe module
├── module_graph.rs               # structure to manage module graph
├── module_side_effects_flag.rs   # handle side-effect flag for module, for tree-shaking
├── parse.rs                      # parse source code with ast.rs
├── plugin.rs                     # plugin driver for compiler
├── plugins                       # builtin plugins
│   ├── assets.rs                 # load assets to js module ast
│   ├── css.rs                    # load css to css module ast
│   ├── javascript.rs             # load js to js module ast
│   ├── json.rs                   # load json to js module ast
│   ├── manifest.rs               # generate manifest file after build
│   ├── md.rs                     # load md and mdx to js module ast
│   ├── minifish_analyze_deps.rs  # special deps analyzer for minifish (extract in the future)
│   ├── minifish_compiler.rs      # special load and generate logic for minifish (extract in the future)
│   ├── mod.rs                    # declare all plugin modules
│   ├── node_polyfill.rs          # modify config for node polyfill
│   ├── runtime.rs                # generate runtime plugins for entry chunk
│   ├── svg.rs                    # load svg to js module ast (SVGR)
│   ├── toml.rs                   # load toml to js module ast
│   ├── wasm.rs                   # load wasm to js module ast
│   ├── xml.rs                    # load xml to js module ast
│   └── yaml.rs                   # load yaml to js module ast
├── reexport_statement_cleanup.rs # cleanup re-export statement, for tree-shaking
├── resolve.rs                    # resolve module by path
├── runtime                       # runtime file templates
│   ├── runtime_async.js          # snippet for require async module
│   ├── runtime_chunk.js          # template to create runtime chunk
│   ├── runtime_entry.js          # template to init runtime module system
│   ├── runtime_hmr.js            # template to create hot update chunk
│   ├── runtime_hmr_entry.js      # snippet for support hmr
│   └── runtime_wasm.js           # snippet for require wasm module
├── sourcemap.rs                  # generate sourcemap
├── statement.rs                  # structure to describe import/export statement
├── statement_graph.rs            # structure to manage import/export statement graph
├── stats.rs                      # create stats info for bundle
├── targets.rs                    # generate swc targets from user config
├── test_helper.rs                # helpers for test
├── transform.rs                  # transform js ast with swc transformers
├── transform_after_resolve.rs
├── transform_async_module.rs
├── transform_css_handler.rs      # transform css ast for replace url and @import
├── transform_css_url_replacer.rs # transform assets to base64 or compiled url for css ast
├── transform_dep_replacer.rs     # transform dep path with runtime module path
├── transform_dynamic_import.rs   # transform dynamic import to runtime require
├── transform_env_replacer.rs     # transform env variables
├── transform_in_generate.rs      # transform ast for generate runtime chunks
├── transform_optimizer.rs        # transform ast for optimize ast
├── transform_provide.rs          # transform ast like webpack provide plugin, for node polyfill
├── transform_react.rs            # transform ast for react component
├── tree_shaking.rs               # implement tree-shaking for compiler
├── tree_shaking_analyze.rs       # analyze import/export statement for tree-shaking
├── tree_shaking_module.rs        # describe module for tree-shaking
├── unused_statement_marker.rs    # mark unused statement with comments, for tree-shaking
├── unused_statement_sweep.rs     # sweep unused statement, for tree-shaking
├── update.rs                     # update module graph after file changed
├── used_ident_collector.rs       # collect used identifiers, for tree-shaking
└── watch.rs                      # watch project file change
```

Flow of mako:

<!-- https://asciiflow.com/#/share/eJzlWE9v2jAU%2FypPPvTUoa4c1vW4HqZJ%2FQiRwAMTPBInikNb1lSaOO%2FAAaEe9hF2mnaq%2BDT9JHUMCYnjgB1o0bSnHBzj9%2F%2B9X565Rwz7BF0iH4%2BCznn77KL9Hp0iD09IJHbvHXTnoMuPH9qnDpqI1fnFmVjF5C4WLw4CY3qeL%2F%2BTx3GYVVjMDxuJ%2B6fUL46hvisIrgI%2FpB6J5JsF7a3cwh9z3pVybTkCjly%2Bo%2F0UFXLzh9kzg35Ebwjc0ngIoTd2KeOphJ9m7MtMK%2FFpvHGm1srcUbi6%2FlJzcvEksssG1LUHHcEq6OuYen3bQJS82jCmEl3CSIRjsicKClF9yuOsabflvBrHxZNBzvVat9VLqQbnaiAWf7bmvDccsxE3zLmu5rf1hwm0Pc%2BnCj8PxlGP%2BDjMzdqIM8WWnJKdaFI%2BAYA5JzHX%2BFAbgylwElHs0e%2Bkak2y0171RC8IJzAQwKhAxuJgnz7p6CfZYYr%2FpSOf1z2jRqjVahWjI5wX4HMCtzjuDRtWSqL9obviNMSxDAxV2U06vuDCPnhR9MKesxKIKcQRIe%2F4EI8oc7PW6ZaNhXSMbPUk%2Bra%2B8YCBYohkyvFVbPzSKqsgybbnUWPIQaI2BT%2Fojz0CboTDYS52VvclTowHgBoPf0v5Og%2BlZZUwNUir9YySFBZVA4CHnvhwr5H8lZQc10v1xGEV7FOrq6KwKDotmP2tKbqZxlSBAZjxQRD569ZY59zGiCl4Ae5nrSU3NIoAhJbC5ARNFJ0U7JVYc8zg12FaTce%2FdSE2D8bBzdikvYQrZkq041VSGi%2FewNcSJZWFaml5yjmQgtIJi4sxS%2F%2FeSe%2FmmT2FpUrqT69%2BBbZoOMvM2sx6MzvxTYxfA2Qnnz0koq0uCB3KBoF8T%2FdlkxTOrWZky0iZj1tLq%2Fns0Uq8faQakj2W2JKDHtDDCzhoQJg%3D) -->
```bash
                                          ┌────────────────────────────────────────────────────────────────────────────┐
                                          │                               incremental compile                          │
                                          │                                                                            │
                                          ▼                                                                            │
                                 ```` Compiler ```````````````````````````````````````````````                         │
                                 `                                                           `                         │
┌─────┐ args ┌────────┐          `    ┌─────────────┐ drive with plugins  ┌────────────┐     `  emit       ┌──────┐    │
│ CLI ├──────► Config ├───────────────►    build    ├─────────────────────►  generate  ├───────────────────► dist │    │
└─────┘      └───▲────┘          `    └───────▲─────┘                     └─────▲──────┘     `  chunks     └───┬──┘    │
                 │               `            │                                 │            `  sourcemaps     │       │
                                 `````````````|`````````````````````````````````|`````````````  assets         │       │
                 │ serialize                  |                                 |               copy files ┌───▼───────┴───┐
                                 ```` Build ``|````````````         ````Generate|`````````````  ...        │ serve & watch │
                 │               `            |           `         `    ┌──────────────┐    `             └───────────────┘
        ┌────────┴─────────┐     `    ┌──────────────┐    `         `    │ tree-shaking │    `
        │ mako.config.json │     `    │    build     │    `         `    └──────────────┘    `
        └──────────────────┘     `    │ module graph ├─┐  `                     |
                                 `    └──────────────┘ │  `         `    ┌──────────────┐    `
                                 `            |        |  `         `    │ split chunks │    `
                                 `            |        |  `         `    └──────────────┘    `
                                 `            |        |  `         `           |            `
                                 `            |        |  `         ` ┌───────────────────┐  `
                                 `    ┌──────────────┐ |  `         ` │ transform modules │  `
                                 `    │ load module  │ |  `         ` │   for generate    │  `
                                 `    │ & transform  │ |  `         ` └───────────────────┘  `
                                 `    └──────────────┘ |  `         `           |            `
                                 `            |        |  `         `  ┌─────────────────┐   `
                                 `            |        |  `         `  │ generate chunks │   `
                                 `````````````|````````|```         `  └─────────────────┘   `
                                              |        |            ````````````|`````````````
                                              |        |                        |
                                 ```` Context ▼````````▼````````````````````````▼`````````````
                                 `                                                           `
                                 `    ┌──────────────┐ ┌─────────────┐ ┌─────────────┐       `
                                 `    │ module_graph │ │ assets_info │ │ chunk_graph │  ...  `
                                 `    └──────────────┘ └─────────────┘ └─────────────┘       `
                                 `                                                           `
                                 `````````````````````````````````````````````````````````````
```

### `node`

`node`(`@okamjs/okam`) is a mixed entity that use to compile mako and distribute it as a node module (base on n-api) for different operation systems, and the summary of directory structure is as follows:

```bashcrates/node
├── build.rs                  # n-api build script for src
├── index.js                  # main entry for npm package
├── npm                       # npm dist for different operation systems
│   ├── darwin-arm64
│   ├── darwin-universal
│   ├── darwin-x64
│   ├── linux-arm-gnueabihf
│   ├── linux-arm64-gnu
│   ├── linux-arm64-musl
│   ├── linux-x64-gnu
│   ├── linux-x64-musl
│   ├── win32-arm64-msvc
│   ├── win32-ia32-msvc
│   └── win32-x64-msvc
├── package.json
├── scripts                   # scripts for development
│   └── release-dev.ts
└── src
    └── lib.rs                # export build function via n-api
```

TIPS: the `okam` is reversal of `mako`.

## `bundler-okam`

`bundler-okam`(`@alipay/umi-bundler-okam`) is a npm package that use to bundle web project by mako, it can be integrated as a bundler to a framework such as Umi, the directory structure is too simple, omitted here.
