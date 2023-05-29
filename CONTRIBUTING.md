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

## Project Structure

There are 2 main crates in this project.

### `mako_bundler`

The core bundler of mako, use to bundle frontend project like webpack. The summary of directory structure is as follows:

```bash
crates/mako_bundler
└── src
    ├── build                   # generate module graph then transform
    │   ├── analyze_deps.rs			# analyze dependency modules for js module
    │   ├── build.rs
    │   ├── load.rs             # load file to js module and collect assets
    │   ├── mod.rs
    │   ├── parse.rs						# parse js module to ast and sourcemap
    │   ├── resolve.rs          # resolve dependency module path
    │   └── transform           # transform js module ast to code
    │       ├── dep_replacer.rs # replace source with id for dependency requires
    │       ├── env_replacer.rs # replace `process.env.xxx` with real value
    │       ├── mod.rs
    │       └── transform.rs
    ├── compiler.rs             # compile project according to config
    ├── config.rs               # normalize bundler config
    ├── context.rs              # compiler context (module graph, assets info, etc.)
    ├── generate                # generate assets, modules and module runtime to disk
    │   ├── generate.rs
    │   └── mod.rs
    ├── lib.rs                  # entry of bundler
    ├── module.rs               # structure for describe each module
    ├── module_graph.rs         # structure for manage module graph (base on petgraph)
    └── utils
        ├── file.rs
        └── mod.rs
```

Flow of bundler:

<!-- https://asciiflow.com/#/share/eJzFVtFqwyAU%2FRXxefRhjNH1cf0NoXHEdgFjijHQrhRGvmAPIx%2Byx9Kv6ZdM06WNqxpN0%2B2SkJvovffIOd64gQynBE5YQekdpHhNOJzADYIrBCdPj%2BM7BNfSux8%2FSE%2BQlZAvCAKDRdLANEuXCSW8fvM2hJgxo%2Bmjl6mMh8%2B9%2B9LrdE43hrYsLDrqjDguAQDCBF8H4jtUO1XkpUhoHLw4S7oFYYRjQdzZ1Mw4yUU9y8BC9dWDhWp3DQuOaC8Wmry%2FJKYX7ZSkPscq%2Bf52RloClvEU0%2BSNXKIoPZCWDnTguRZVUxVcFtdD7MUjYLBjcd9ddEoTqug6sNlh04zNk4VGUZNUPU676KeKfLw77o%2FrlwLSLC4oAQuOl6%2BgBddISu%2F1m6lx0G8O6AQXkEu5nOD4XxEMXNzCQTk8N3Ww4Jjlc9kAAPCVzoXeLIoOg9JrQ1oTmgfaPPiBs5oaLZax%2FMNZQh3Wbu4t3w7Dlavat31XK5aNSx3JQv8agx%2B3hu9Nvc5j%2FYR11v9M67fHEZznROSzhM0zfWA0Gt2sM%2F%2Fh6vvZDY4vEYJbuP0G7CrIaQ%3D%3D) -->
```bash
                    ```` Compiler `````````````````````````````````````````````
                    `                                                         `
┌─────────┐         `    ┌─────────────┐                    ┌────────────┐    `   ┌────────┐
│  entry  ├──────────────►    build    ├────────────────────►  generate  ├────────►  dist  │
└────▲────┘         `    └──────▲──────┘                    └─────▲──────┘    `   └────────┘
     │              `           │                                 │           `
                    ```````````````````````````````````````````````````````````
     │ normalize                │                                 │
                    ```` Build ```````` ``````
     │              `           │            `                    │
┌────┴─────┐        `    ┌──────┴───────┐    `
│  Config  │        `    │    build     ├─ ─ ─ ─ ─ ─ ─ ─ ┐        │
└──────────┘        `    │ module graph │    `
                    `    └──────┬───────┘    `           │        │
                    `           │            `
                    `                        `           │        │ read
                    `           │            `
                    `                        `           │        │
                    `           │            `
                    `    ┌──────┴───────┐    `           │        │
                    `    │  transform   │    `
                    `    │ module graph ├─ ─ ─ ┐         │        │
                    `    └──────────────┘    `
                    `                        ` │         │        │
                    ``````````````````````````    update
                                               │         │        │
                                               ▼         ▼
                    ```` Context ```````````````````````````````````````````````
                    `                                                          `
                    `    ┌──────────────┐      ┌─────────────┐      ┌─────┐    `
                    `    │ module_graph │      │ assets_info │      │ ... │    `
                    `    └──────────────┘      └─────────────┘      └─────┘    `
                    `                                                          `
                    ````````````````````````````````````````````````````````````
```

### `mako_cli`

The CLI of mako, use to drive bundler via command line.

Currently too simple, details omitted.
