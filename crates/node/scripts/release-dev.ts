import 'zx/globals';

(async () => {
  // Check git status
  console.log('Check git status');
  const status = (await $`git status --porcelain`).stdout.trim();
  if (status) {
    throw new Error('Please commit all changes before release');
  }

  // bump version
  console.log('Bump version');
  const nodePkgDir = path.resolve(__dirname, '..');
  const dist = path.join(nodePkgDir, 'dist');
  const nodePkgPath = path.join(nodePkgDir, 'package.json');
  const distPkgPath = path.join(nodePkgDir, 'dist', 'package.json');
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

  if (tag !== 'dev' && tag !== 'canary') {
    console.log('Please specify version like  x.y.z-canary.n or x.y.z-dev.n');
    throw Error('Only dev and canary tags are allowed');
  }

  nodePkg.version = newVersion;
  nodePkg.optionalDependencies = {};

  await $`mkdir -p dist`;
  await $`rm -rf dist/*`;

  fs.writeFileSync(distPkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  // build macOs *.node
  await $`rm -rf ./*.node`;
  await $`pnpm run build:mac:x86`;
  await $`pnpm run build:mac:aarch`;
  await $`strip -x ./*.node`;
  await $`mv *.node     dist`;
  await $`cp index.js   dist`;
  await $`cp index.d.ts dist`;

  // --ignore-scripts because we don't publish optional pkg
  await $`cd dist && npm publish --ignore-scripts --tag ${tag} --access public`;
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
