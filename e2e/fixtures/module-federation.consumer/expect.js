const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const manifest = JSON.parse(files["mf-manifest.json"]);

assert(
  manifest.remotes[0].alias === 'producer'
  && manifest.remotes[0].federationContainerName === 'producer'
  && manifest.remotes[0].moduleName === 'App',
  "should include mf remotes info"
)

assert(
  manifest.shared.map(s => s.name).sort().join(",") === "react,react-dom",
  "should include mf shared dependencies"
)

assert(
  manifest.shared.every(s => s.assets.js.sync.length !== 0),
  "should include mf shared assets"
)
