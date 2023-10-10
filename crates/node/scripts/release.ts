import 'zx/globals';
import * as process from 'process';

(async () => {
  console.log('Check branch');
  const branch = (await $`git branch --show-current`).stdout.trim();
  if (branch !== 'master') {
    throw new Error('Please run this script in master branch');
  }

  // Check git status
  console.log('Check git status');
  const status = (await $`git status --porcelain`).stdout.trim();
  if (status) {
    throw new Error('Please commit all changes before release');
  }

  // bump version
  console.log('Bump version');
  const nodePkgDir = path.resolve(__dirname, '..');
  const nodePkgPath = path.join(nodePkgDir, 'package.json');
  const nodePkg = JSON.parse(fs.readFileSync(nodePkgPath, 'utf-8'));
  const currentVersion = nodePkg.version;

  console.log('current version: ', currentVersion);
  const newVersion = (await question(`What's next version? `)).trim();

  let tag = 'latest';
  if (
    newVersion.includes('-alpha.') ||
    newVersion.includes('-beta.') ||
    newVersion.includes('-rc.')
  )
    tag = 'next';
  if (newVersion.includes('-canary.')) tag = 'canary';
  if (newVersion.includes('-dev.')) tag = 'dev';

  nodePkg.version = newVersion;

  console.log(`${nodePkg.name}@${newVersion} will be published`);
  const willContinue = ((await question(`Continue? y/[n]`)) || 'n').trim();

  if (willContinue !== 'y') {
    console.log('Abort!');
    process.exit(0);
  }

  fs.writeFileSync(nodePkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  // build macOs *.node
  await $`rm -rf ./*.node`;
  await $`pnpm run build:mac:x86`;
  await $`pnpm run build:mac:aarch`;

  // ref https://gist.github.com/shqld/256e2c4f4b97957fb0ec250cdc6dc463
  $.env.CC_X86_64_UNKNOWN_LINUX_GNU = 'x86_64-unknown-linux-gnu-gcc';
  $.env.CXX_X86_64_UNKNOWN_LINUX_GNU = 'x86_64-unknown-linux-gnu-g++';
  $.env.AR_X86_64_UNKNOWN_LINUX_GNU = 'x86_64-unknown-linux-gnu-ar';
  $.env.CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER =
    'x86_64-unknown-linux-gnu-gcc';
  await $`pnpm run build:linux:x86`;

  await $`strip -x ./okam.darwin-*.node`;
  await $`docker run  --rm   -v $PWD:/workspace -w /workspace  ghcr.io/napi-rs/napi-rs/nodejs-rust:lts-debian   bash -c "strip okam.linux-x64-gnu.node"`;

  await $`pnpm run artifacts:local`;

  // --ignore-scripts because we don't publish optional pkg
  await $`npm publish --tag ${tag} --access public`;

  // set new version to bundler-okam
  console.log('Set new version to bundler-okam');
  const bundlerOkamPkgPath = path.join(
    __dirname,
    '../../../packages/bundler-okam/package.json',
  );
  const bundlerOkamPkg = JSON.parse(
    fs.readFileSync(bundlerOkamPkgPath, 'utf-8'),
  );
  bundlerOkamPkg.dependencies['@okamjs/okam'] = `${newVersion}`;
  fs.writeFileSync(
    bundlerOkamPkgPath,
    JSON.stringify(bundlerOkamPkg, null, 2) + '\n',
  );

  await $`git commit -a -m "release: ${nodePkg.name}@${newVersion}"`;
  // tag
  console.log('Tag');
  await $`git tag v${newVersion}`;

  // push
  console.log('Push');
  await $`git push origin ${branch} --tags`;
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
