const { build } = require('@okamjs/okam');
const { parseServerStats } = require('@okamjs/rsc');
const path = require('path');
const fs = require('fs');
const { platform } = require('os');

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
      output: {
        path: serverOutputPath,
      },
      rscServer: {
        clientComponentTpl: `
module.exports = {$$typeof: Symbol.for(\"react.module.reference\"),filepath:\"{{path}}\",name:\"*\"};
        `,
        emitCSS: true,
      },
      umd: '__rsc_server__',
      platform: 'node',
      stats: true,
    },
    hooks: {},
    watch: false,
  });

  // build client
  fs.rmdirSync(path.join(root, 'tmp'), { recursive: true });
  fs.mkdirSync(path.join(root, 'tmp'));
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
      stats: true,
      umd: '__rsc_client__',
      rscServer: false,
      rscClient: {
        // TODO: remove this
        x: 1,
      },
      mode: 'production',
    },
    hooks: {},
    watch: false,
  });
})();
