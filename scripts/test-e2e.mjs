import test from 'node:test';
import 'zx/globals';

function winPath(path) {
  const isExtendedLengthPath = /^\\\\\?\\/.test(path);
  if (isExtendedLengthPath) {
    return path;
  }

  return path.replace(/\\/g, '/');
}

function winJoin(...args) {
  return winPath(path.join(...args));
}

// node version 小于 20 时退出
const nodeVersion = process.versions.node.split('.')[0];
if (nodeVersion < 20) {
  console.log('node version must >= 20');
  process.exit(1);
}

const root = process.cwd();
const fixtures = winJoin(root, argv.fixtures || 'e2e/fixtures');
let onlyDir = argv.only ? argv.only : null;
const dirs = fs.readdirSync(fixtures).filter((dir) => {
  if (dir.endsWith('-only')) {
    onlyDir = dir;
  }
  return (
    !dir.startsWith('.') &&
    fs.statSync(winJoin(fixtures, dir)).isDirectory() &&
    fs.existsSync(winJoin(fixtures, dir, 'expect.js'))
  );
});

for (const dir of onlyDir ? [onlyDir] : dirs) {
  const testFn = dir.includes('failed') && !argv.only ? test.skip : test;
  await testFn(dir, async () => {
    const cwd = winJoin(fixtures, dir);
    if (argv.umi) {
      if (!fs.existsSync(winJoin(cwd, 'node_modules'))) {
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
        await $`node ${winJoin(root, 'scripts', 'mako.js')} ${cwd}`;
      } catch (e) {
        const isErrorCase = dir.split('.').includes('error');
        if (isErrorCase) {
          const mod = await import(winJoin(fixtures, dir, 'expect.js'));
          mod.default(e);
          return;
        } else {
          throw e;
        }
      }
    }
    // run expect.js
    const mod = await import(winJoin(fixtures, dir, 'expect.js'));
    if (mod && typeof mod.default === 'function') {
      await mod.default();
    }
    if (dir === 'config.targets.runtime') {
      await $`npx es-check es5 ./e2e/fixtures/config.targets.runtime/dist/*.js`;
    }
  });
}
