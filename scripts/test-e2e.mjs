import net from 'net';
import test from 'node:test';
import 'zx/globals';

const defaultPort = 8000;
function checkServer(port, host, callback) {
  const client = new net.Socket();

  client.connect({ port, host }, () => {
    client.end();
    callback(true);
  });

  client.on('error', () => {
    client.destroy();
    callback(false);
  });
}

async function waitForServer(port, host, interval, maxAttempts) {
  return new Promise((resolve) => {
    let attempts = 0;

    const intervalId = setInterval(() => {
      if (attempts >= maxAttempts) {
        clearInterval(intervalId);
        resolve(false);
        return;
      }

      checkServer(port, host, (isRunning) => {
        if (isRunning) {
          clearInterval(intervalId);
          resolve(true);
        } else {
          attempts++;
        }
      });
    }, interval);
  });
}

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
    (fs.existsSync(path.join(fixtures, dir, 'expect.js')) ||
      fs.existsSync(path.join(fixtures, dir, 'expect.mjs')))
  );
});
// import expect.mjs or expect.js
async function runExpect(dir, error) {
  const expectPath = `file://${path.join(fixtures, dir, 'expect.js')}`;
  const mod = await import(expectPath);
  if (mod && typeof mod.default === 'function') {
    await mod.default(error);
  }
}
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
      // 如果目录名以dev开头,则运行dev命令否则运行build命令
      if (dir.startsWith('dev')) {
        console.log(`cd ${cwd} && umi dev`);
        let p = $.spawn('sh', ['-c', `cd ${cwd} && OKAM=${x} umi dev`], {
          stdio: 'inherit',
        });
        const isRunning = await waitForServer(
          defaultPort + 1, // mako's port, when it's open, dev can serve
          'localhost',
          1000,
          30,
        );
        if (isRunning) {
          console.log(`Server is running on port ${defaultPort}`);
          try {
            await runExpect(dir);
          } catch (e) {
            console.log('dev error', e);
            throw e;
          } finally {
            p.kill(9);
          }
        } else {
          console.log(`Failed to connect to server on port ${defaultPort}`);
        }
        return;
      } else {
        console.log(`cd ${cwd} && COMPRESS=none OKAM=${x} umi build`);
        await $`cd ${cwd} && COMPRESS=none OKAM=${x} umi build`;
      }
    } else {
      try {
        // run mako build
        await $`node ${path.join(root, 'scripts', 'mako.js')} ${cwd}`;
      } catch (e) {
        const isErrorCase = dir.split('.').includes('error');
        if (isErrorCase) {
          await runExpect(dir, e);
          return;
        } else {
          throw e;
        }
      }
    }
    // run expect.js
    await runExpect(dir);
    if (dir === 'config.targets.runtime') {
      await $`npx es-check es5 ./e2e/fixtures/config.targets.runtime/dist/*.js`;
    }
  });
}
