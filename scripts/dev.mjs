import "zx/globals";
import chokidar from "chokidar";
import path from "path";
import fs from "fs";
import assert from "assert";

const targetDir = process.argv.slice(2)[0];
const cwd = process.cwd();
const makoPath = path.join(cwd, "target/release/mako");

assert(targetDir, "targetDir is required, e.g. `pnpm dev examples/with-antd`");
assert(
  fs.existsSync(makoPath),
  "mako not found, please run `cargo build --release` first"
);

console.log("watch", targetDir);
const watcher = chokidar.watch([`${targetDir}/**/*.(js|jsx|ts|tsx)`], {
  ignoreInitial: true,
  ignored: [/node_modules/, /dist/],
});
watcher.on("all", async (event, changedPath) => {
  console.log(1, event, changedPath);
  build().catch((e) => {
    console.error(e);
  });
});

build().catch((e) => {
  console.error(e);
});

async function build() {
  await $`${makoPath} ${targetDir}`;
}
