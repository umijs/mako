import 'zx/globals';
import assert from 'assert';
import { spawn } from 'child_process';
import os from 'os';
import { chromium, devices } from 'playwright';
import waitPort from 'wait-port';

(async () => {
  const tmpDir = os.tmpdir();
  const root = path.join(tmpDir, 'mako-check-ecosystem-usages');
  await $`mkdir -p ${root}`;
  const makoVersion = require('../packages/mako/package.json').version;
  await checkDumiWithAntDesign({ root, makoVersion });
})().catch(console.error);

interface CheckOptions {
  root: string;
  makoVersion: string;
}

async function checkDumiWithAntDesign(opts: CheckOptions) {
  console.log('checkDumiWithAntDesign', opts);
  const cwd = path.join(opts.root, 'ant-design');
  // git clone
  if (!fs.existsSync(cwd)) {
    console.log('git clone');
    await $`cd ${opts.root} && git clone git@github.com:ant-design/ant-design.git --depth 1`;
  } else {
    console.log('target dir exists, skip git clone');
  }
  console.log('add @umijs/mako to resolutions');
  const pkg = JSON.parse(
    fs.readFileSync(path.join(cwd, 'package.json'), 'utf-8'),
  );
  // add resolutions
  pkg.resolutions = {
    ...pkg.resolutions,
    '@umijs/mako': opts.makoVersion,
    '@umijs/bundler-mako': opts.makoVersion,
  };
  fs.writeFileSync(
    path.join(cwd, 'package.json'),
    JSON.stringify(pkg, null, 2),
  );
  // pnpm install
  await $`cd ${cwd} && pnpm install`;
  // pnpm start
  // await $`cd ${cwd} && pnpm start`;
  const child = spawn('pnpm', ['start'], {
    cwd,
    stdio: 'inherit',
    shell: true,
    detached: true,
  });
  // make sure localhost:8001 is ok with playwright
  console.log('wait for port 8001 is ready');
  // wait 35s
  await waitPort({ port: 8001, timeout: 35000 });
  console.log('port 8001 is ready');
  // wait 35s
  console.log('wait 35s');
  await new Promise((resolve) => setTimeout(resolve, 35000));
  console.log('test http://localhost:8001 with playwright');
  const browser = await chromium.launch();
  const context = await browser.newContext(devices['iPhone 11']);
  const page = await context.newPage();
  await page.goto(`http://localhost:8001`);
  await page.waitForTimeout(10000);
  const el = await page.$('#root');
  assert(el, 'root element not found');
  const html = await el.innerHTML();
  assert(
    html.includes('Ant Design'),
    'Ant Design not found in the #root element',
  );
  console.log('test passed');
  await browser.close();
  child.kill();
}
