import test from 'node:test';
import 'zx/globals';

// node version 小于 20 时退出
const nodeVersion = process.versions.node.split('.')[0];
if (nodeVersion < 20) {
  console.log('node version must >= 20');
  process.exit(1);
}

const root = process.cwd();
const fixtures = path.join(root, argv.fixtures || 'e2e/fixtures');
let onlyDir = argv.only ? argv.only : null;
const dirs = fs.readdirSync(fixtures).filter((dir) => {
  if (dir.endsWith('-only')) {
    onlyDir = dir;
  }
  return (
    !dir.startsWith('.') &&
    fs.statSync(path.join(fixtures, dir)).isDirectory() &&
    fs.existsSync(path.join(fixtures, dir, 'expect.js'))
  );
});

for (const dir of onlyDir ? [onlyDir] : dirs) {
  const testFn = dir.includes('failed') && !argv.only ? test.skip : test;
  await testFn(dir, async () => {
    const cwd = path.join(fixtures, dir);
    if (argv.umi) {
      if (!fs.existsSync(path.join(cwd, 'node_modules'))) {
        await $`cd ${cwd} && mkdir node_modules`;
      }
      // run umi build
      const x = (await import.meta.resolve('@umijs/bundler-mako')).replace(
        /^file:\/\//,
        '',
      );
      console.log(`cd ${cwd} && COMPRESS=none OKAM=${x} umi build`);
      await $`cd ${cwd} && COMPRESS=none OKAM=${x} umi build`;
    } else {
      try {
        // run mako build
        await $`node ${path.join(root, 'scripts', 'mako.js')} ${cwd}`;
      } catch (e) {
        const isErrorCase = dir.split('.').includes('error');
        if (isErrorCase) {
          const mod = await import(path.join(fixtures, dir, 'expect.js'));
          mod.default(e);
          return;
        } else {
          throw e;
        }
      }
    }
    // run expect.js
    const mod = await import(path.join(fixtures, dir, 'expect.js'));
    if (mod && typeof mod.default === 'function') {
      await mod.default();
    }
    if (dir === 'config.targets.runtime') {
      await $`npx es-check es5 ./e2e/fixtures/config.targets.runtime/dist/*.js`;
    }
  });
}
