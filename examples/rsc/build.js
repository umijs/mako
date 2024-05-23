const { build } = require('@umijs/mako');
const { parseServerStats } = require('@umijs/rsc');
const path = require('path');
const fs = require('fs');

(async () => {
  const root = process.cwd();

  // build server
  let serverOutputPath = path.join(root, 'server');
  await build({
    root,
    config: {
      mode: 'production',
      minify: false,
      entry: {
        index: path.join(root, 'src/index.tsx'),
        runtime: path.join(root, 'src/server-runtime.tsx'),
      },
      // moduleIdStrategy: 'hashed',
      output: {
        path: serverOutputPath,
      },
      rscServer: {
        clientComponentTpl: `
module.exports = {$$typeof: Symbol.for(\"react.module.reference\"),filepath:\"{{id}}\",_path:\"{{path}}\",name:\"*\"};
        `,
        emitCSS: true,
      },
      umd: '__rsc_server__',
      platform: 'node',
      stats: {
        modules: true,
      },
    },
    hooks: {},
    watch: false,
  });

  // build client
  let tmpDir = path.join(root, 'tmp');
  if (fs.existsSync(tmpDir)) {
    fs.rmdirSync(tmpDir, { recursive: true });
  }
  fs.mkdirSync(tmpDir);
  let stats = fs.readFileSync(
    path.join(serverOutputPath, 'stats.json'),
    'utf-8',
  );
  stats = JSON.parse(stats);
  const { rscClientComponents } = parseServerStats(stats);
  let clientCode = rscClientComponents
    .map((c) => {
      return `import('../${c}');`;
    })
    .join('\n');
  fs.writeFileSync(
    path.join(root, 'tmp/index.tsx'),
    `
export default () => {
  ${clientCode}
}
  `,
  );
  // process.env.DUMP_MAKO_CONFIG = 1;
  await build({
    root,
    config: {
      entry: {
        index: path.join(root, 'tmp/index.tsx'),
      },
      platform: 'node',
      stats: {
        modules: false,
      },
      umd: '__rsc_client__',
      rscServer: false,
      rscClient: {
        logServerComponent: 'error',
      },
      mode: 'production',
      // moduleIdStrategy: 'hashed',
    },
    hooks: {},
    watch: false,
  });
})();
