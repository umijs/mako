# Contributing

## Getting Started

Clone.

```bash
$ git clone git@github.com:umijs/mako.git
$ cd mako
```

Install tools.

```bash
$ cargo install just
$ cargo install cargo-binstall
```

Compile.

```bash
$ cargo build
$ cargo build --release
```

Run.

```bash
$ cargo run --bin mako examples/normal
# with HMR
$ cargo run --bin mako examples/normal --watch
# in production
$ cargo run --bin mako examples/normal --mode production
# filter logs
$ RUST_LOG=mako=debug,info cargo run --bin mako examples/normal
$ RUST_LOG=mako::parse=debug,info cargo run --bin mako examples/normal
```

## Advanced Tasks

Before you push.

```bash
$ just ready
# without e2e
$ just ready-lite
```

Test.

```bash
$ pnpm playwright install # only need to run before the first time you run "jest test"
$ just test
# test specified testcase
$ cargo nextest run transformers::transform_try_resolve::tests
# review snapshot
$ cargo insta review
```

Coverage.

```bash
$ cargo codecov
$ cargo codecov --html && open target/llvm-cov/html/index.html
```

Format.

```bash
$ just fmt
```

Lint.

```bash
$ just lint
```

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

Benchmark three10x.

```bash
$ just setup-bench
# default: --baseline master --case tmp/three10x --warmup 3 --runs 10
$ just bench
$ just bench --multiChunks
$ just bench --baseline v0.4.4
$ just bench --baseline v0.4.4 --case examples/with-antd
$ just bench --no-build
```

Performance analysis with puffin.

```bash
$ cargo build --release --features profile
$ ./target/release/mako examples/normal --mode=production
```

Performance analysis with [Xcode instruments](https://help.apple.com/instruments/mac).

- Install Xcode App from Mac AppStore and switch xcode dev tool with:

```bash
$ sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
```

- Install [cargo-instruments](https://crates.io/crates/cargo-instruments) with:

```bash
$ cargo install cargo-instruments

# see instruments template:
$ cargo instruments --list-templates
```

- capture instruments trace with:

```bash
$ cargo instruments -t sys --profile release-debug --package mako --bin mako examples/with-antd
```

- you can open the trace file with instruments again with:

```bash
$ open target/instruments/mako_System-Trace_xxx.trace
```

Use mako in umi or bigfish.

```bash
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js umi build
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js umi dev
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js max build
$ OKAM=/PATH/TO/umijs/marko/packages/bundler-okam/index.js bigfish build
```

## Release

You can release mako with ci or locally.

### Release with CI

> NOTICE: _canary_ and _dev_ tags are now supported to be released with CI.

```bash
# Make sure everything is ok
$ just ready
# Release with CI
$ npm run release
# After released successful, you need to release bundler-okam manually.
$ npm run release:bundler-okam
```

### Release Locally

Refer to https://yuque.antfin.com/mako/vz2gn4/vkp4qs8u4zcuxqoc for details.
