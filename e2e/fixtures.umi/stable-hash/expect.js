const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");


function getEntryHash() {
  const { files } = parseBuildResult(__dirname);
  return Object.keys(files).find((f) => f.startsWith("umi."))?.replace(/^umi\.(.*)\.js$/, "$1")
}

const test = async() => {
  await import('zx');
  const firstHash = getEntryHash();

  if (!fs.existsSync(path.join(__dirname, 'node_modules'))) {
    await $`cd ${__dirname} && mkdir node_modules`;
  }
  // run umi build
  const x = require.resolve('@umijs/bundler-mako').replace(
    /^file:\/\//,
    '',
  );
  console.log(`cd ${__dirname} && COMPRESS=none OKAM=${x} umi build`);
  await $`cd ${__dirname} && COMPRESS=none OKAM=${x} umi build`;

  const secondHash = getEntryHash();

  console.table({firstHash, secondHash})

  assert(firstHash === secondHash, "chunk hash is stable");
}

module.exports = test;

