#!/usr/bin/env -S just --justfile

_default:
  just --list -u

alias r := ready
alias c := codecov
alias t := test

# Initialize the project by installing all the necessary tools.
# Make sure you have cargo-binstall installed.
# You can download the pre-compiled binary from <https://github.com/cargo-bins/cargo-binstall#installation>
# or install via `cargo install cargo-binstall`
init:
  cargo binstall cargo-nextest cargo-watch cargo-insta typos-cli taplo-cli cargo-llvm-cov -y

# When ready, run the same CI commands
ready-lite:
  typos
  cargo fmt
  just check
  just test
  just lint
  pnpm biome:format
  git status

ready:
  typos
  cargo fmt
  just check
  just test
  just lint
  cargo build --release
  pnpm --filter @okamjs/okam build
  pnpm --filter @okamjs/okam format:dts
  pnpm test
  pnpm biome:format
  git status

# Update our local branch with the remote branch (this is for you to sync the submodules)
update:
  git pull
  git submodule update --init

# Run `cargo watch`
# --no-vcs-ignores: cargo-watch has a bug loading all .gitignores, including the ones listed in .gitignore
# use .ignore file getting the ignore list
watch command:
  cargo watch --no-vcs-ignores -x '{{command}}'

# Format all files
fmt:
  cargo fmt
  taplo format

# Run cargo check
check:
  cargo check --locked

# Run all the tests
test:
  cargo nextest run

# Lint the whole project
lint:
  cargo clippy --locked -- --deny warnings

# Get code coverage
codecov:
  cargo codecov --html

cli +args:
  cargo run --bin mako -- {{args}}

setup-bench:
  git clone --branch r108 --depth 1 git@github.com:mrdoob/three.js.git ./tmp/three
  echo "import * as three from './src/Three.js'; export { three }" > tmp/three/entry.ts
  mkdir -p tmp/three10x
  for i in {1..10}; do cp -r ./tmp/three/src ./tmp/three10x/copy$i/; done
  echo > tmp/three10x/index.ts
  for i in {1..10}; do echo "import * as three$i from './copy$i/Three.js'; export { three$i }" >> tmp/three10x/index.ts; done

bench +args='':
  npm run benchmark -- {{args}}
