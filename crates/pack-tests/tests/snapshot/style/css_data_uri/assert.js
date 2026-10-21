const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

// The fixture writes each endpoint before taking snapshots. Development assets
// live outside the endpoint's dist root, so their contents are checked here.
const outputDir = path.join(__dirname, "output");

function outputFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const filename = path.join(directory, entry.name);
    return entry.isDirectory() ? outputFiles(filename) : [filename];
  });
}

const assets = outputFiles(outputDir);
const css = fs.readFileSync(path.join(outputDir, "styles.css"), "utf8");
const png =
  "image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=";

for (const scheme of ["data:", "DATA:"]) {
  assert(
    css.includes(`${scheme}${png}`),
    `CSS must preserve the ${scheme} URI instead of resolving a local asset`,
  );
}

const longSvg =
  "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22%3E%3C%21--LongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegressionLongDataUriRegression--%3E%3C/svg%3E";
assert(
  css.includes(longSvg),
  "long data URIs must stay inline when development endpoints are watched",
);

// The fallback makes an incorrect URI lookup emit a local asset on every
// platform, without relying on Linux's watcher rejecting an oversized filename.
for (const asset of assets) {
  assert(
    !fs
      .readFileSync(asset, "utf8")
      .includes("DATA_URI_MUST_NOT_RESOLVE_TO_THIS_LOCAL_FILE"),
    "data URIs must bypass the wildcard alias fallback",
  );
}

assert(
  assets
    .filter((asset) => asset.endsWith(".js"))
    .some((asset) =>
      fs.readFileSync(asset, "utf8").includes("WILDCARD_PATHS_APPLIED"),
    ),
  "ordinary module requests must still resolve through tsconfig paths",
);

const relativeAsset = assets.find(
  (asset) =>
    asset.endsWith(".svg") &&
    fs.readFileSync(asset, "utf8").includes("RELATIVE_ASSET_EMITTED"),
);

assert(relativeAsset, "relative CSS assets must still be emitted");
assert(
  css.includes(path.relative(outputDir, relativeAsset).split(path.sep).join("/")),
  "relative CSS URLs must reference the emitted asset",
);
