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
```

Format.

```bash
$ cargo fmt
```

Lint.

```bash
$ cargo lint
```

Benchmark with-antd example.

```bash
$ cargo build --release
$ time ./target/release/mako examples/with-antd
# or using hyperfine
$ hyperfine --runs 10 "./target/release/mako examples/with-antd"
```

Use mako in umi or bigfish.

```bash
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js umi build --dev
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js max build --dev
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js bigfish build --dev
```

## Project Structure

There are 1 main crate `mako` in this project, and the summary of directory structure is as follows:

```bash
crates/mako/src
├── analyze_deps.rs               # analyze deps from js/css ast
├── ast.rs                        # parse source to ast and parse ast to code and sourcemap
├── bfs.rs                        # util for breadth-first search
├── build.rs                      # transform source code to ast and combine into module graph
├── chunk.rs                      # structure to describe chunk
├── chunk_graph.rs                # structure to manage chunk graph
├── cli.rs                        # cli arguments parser
├── compiler.rs                   # compile project according to user config
├── config.rs                     # serialize and watch user config
├── config_node_polyfill.rs       # externalize node standard library
├── copy.rs                       # util for copy files to dist
├── generate.rs                   # generate modules and assets to dist
├── generate_chunks.rs            # generate chunks from module graph and chunk graph
├── group_chunk.rs                # split module graph into chunks
├── load.rs                       # load file content or base64 (assets only)
├── main.rs                       # the entry of this crate
├── minify.rs                     # minify js code via ast
├── module.rs                     # structure to describe module
├── module_graph.rs               # structure to manage module graph
├── parse.rs                      # parse source code with ast.rs
├── resolve.rs                    # resolve module by path
├── runtime                       # runtime file templates
│   ├── runtime_chunk.js          # template to create runtime chunk
│   ├── runtime_css.ts            # template to apply inline css module in runtime
│   └── runtime_entry.js          # template to init runtime module system
├── sourcemap.rs                  # generate sourcemap
├── transform.rs                  # transform js ast with swc transformers
├── transform_css_handler.rs      # transform css ast for replace url and @import
├── transform_dep_replacer.rs     # transform dep path with runtime module path
├── transform_dynamic_import.rs   # transform dynamic import to runtime require
├── transform_env_replacer.rs     # transform env variables
├── transform_in_generate.rs      # transform ast for generate runtime chunks
├── transform_optimizer.rs        # transform ast for optimize ast
└── watch.rs                      # todo
```

Flow of mako:

<!-- https://asciiflow.com/#/share/eJzFV71u2zAQfhWCQ6dAQ1ugacZmKAr0EQTYjE07aiRSIGU0zg9gZO7gwTA69BE6FZkKP42fpBQl64c5SqRspAcBpqU73ne%2FPN5jRhKKL3BCbviI0e%2F4DMdkSYV6dR%2Fi2xBffPzw7izES7V6e%2F5erTJ6m6k%2FIUZ9NFaELnmSRjEV%2Bp8HhSHrV9DL0SGrFOw3u5cPImIuEfip5mlD0C9Xbs8aRLPf%2FHAT3x200iTKKmEYZWEhuvz6xcKy%2FavCw2bRvMdcWFTR1SKKp74eaJlTC%2BY7zimjgmTUbpMruGkkM1T5wBZBQ8ohyrC6rgxpRXljemD7pzPKk%2BsFu5F9UQYUmQDG3Z8hrE%2BGvOQLMaEJSaVTbTbJt%2FgRIlLSDFSUA5NURCSO7mgvbhfbFE14ukQz1aecbUOfdO4btrVYPh%2By2bAtCIKjgzYGmcygNdQcU1C1niGSBlRLq3juaniGISg%2FrYKJbl3BN8kZMkBqL1TNSb34BQNBMo1VGy2rDFZ2Eq89oYRPFzFFc0HS62rbNewdXwc7n5VAA8qf35aY%2FCxcYkAs6aG1u%2FdxbU%2FnY3xe4B0%2BO%2BjUMcCgTBAmZ1wkZRDLTDmxzepX6WgcgegEaiB7Xsu5lpnm2ZJsa9MnNQ1Mtdz4mJPpofb0ixceGe4SbzBvGqkENqV80yoBWl3p6BpfddT4%2F%2FHIgexVYeM5aYXbpL2HHLVzc94BmRo8mlyUwLSHJ78%2BGWA5cGdtp7o%2B5PdBtN%2FuKhPrpUnmp9e447kp8E505wub7my%2BReQNvmxxo2q80HleDNGjiM142ft2RVdp8OXTqLenHA1fFUBs%2FcgycHjs7eupgeTfC3wpxI%2F48R%2BiZatF) -->
```bash
                                 ```` Compiler ```````````````````````````````````````````````
                                 `                                                           `
┌─────┐ args ┌────────┐          `    ┌─────────────┐                     ┌────────────┐     `  emit       ┌──────┐
│ CLI ├──────► Config ├───────────────►    build    ├─────────────────────►  generate  ├───────────────────► dist │
└─────┘      └───▲────┘          `    └───────▲─────┘                     └─────▲──────┘     `  chunks     └──────┘
                 │               `            │                                 │            `  sourcemaps
                                 ````````````` ```````````````````````````````````````````````  assets
                 │ serialize                  │                                 │               copy files
                                 ```` Build `` ````````````         ````Generate `````````````  ...
                 │               `            │           `         `           │            `
        ┌────────┴─────────┐     `    ┌───────┴──────┐    `         `    ┌──────┴───────┐    `
        │ mako.config.json │     `    │    build     │    `         `    │ split chunks │    `
        └──────────────────┘     `    │ module graph ├─┐  `         `    └──────────────┘    `
                                 `    └───────┬──────┘ │  `         `           |            `
                                 `            │           `         ` ┌───────────────────┐  `
                                 `                     │  `         ` │ transform modules │  `
                                 `            │           `         ` │   for generate    │  `
                                 `            │        │  `         ` └───────────────────┘  `
                                 `    ┌───────┴──────┐    `         `           |            `
                                 `    │ load module  │ │  `         `  ┌─────────────────┐   `
                                 `    │ & transform  │    `         `  │ generate chunks │   `
                                 `    └───────┬──────┘ │  `         `  └────────┬────────┘   `
                                 `            │           `         `           │            `
                                 `                     │  `         `                        `
                                 `````````````│```````` ```         ````````````│`````````````
                                                       │
                                              │        │                        │
                                 ```` Context ▼````````▼````````````````````````▼`````````````
                                 `                                                           `
                                 `    ┌──────────────┐ ┌─────────────┐ ┌─────────────┐       `
                                 `    │ module_graph │ │ assets_info │ │ chunk_graph │  ...  `
                                 `    └──────────────┘ └─────────────┘ └─────────────┘       `
                                 `                                                           `
                                 `````````````````````````````````````````````````````````````
```
