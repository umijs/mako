const assert = require("assert");
const { parseBuildResult } = require("../../../scripts/test-utils");
const { files } = parseBuildResult(__dirname);

const manifest = JSON.parse(files["mf-manifest.json"]);

assert(
  manifest.metaData.remoteEntry.name === 'remoteEntry.js',
  "should generate mf contanier entry"
)

assert(
  manifest.exposes[0].name === 'App',
  "should include mf exposes"
)

assert(
  manifest.exposes[0].assets.js.sync.length !== 0,
  "should include mf exposes assets"
)

assert(
  manifest.shared.map(s => s.name).sort().join(",") === "react,react-dom",
  "should include mf shared dependencies"
)

assert(
  manifest.shared.every(s => s.assets.js.sync.length !== 0),
  "should include mf shared assets"
)
