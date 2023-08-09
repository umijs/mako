import 'zx/globals';

(async () => {
  // Check branch
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
  const nodePkgDir = path.join(__dirname, '../crates/node');
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

  if (tag !== 'dev' && tag !== 'canary') {
    console.log('Please specify version like  x.y.z-canary.n or x.y.z-dev.n');
    throw Error('Only dev and canary tags are allowed');
  }

  nodePkg.version = newVersion;
  fs.writeFileSync(nodePkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  // set new version to bundler-okam
  console.log('Set new version to bundler-okam');
  const bundlerOkamPkgPath = path.join(
    __dirname,
    '../packages/bundler-okam/package.json',
  );
  const bundlerOkamPkg = JSON.parse(
    fs.readFileSync(bundlerOkamPkgPath, 'utf-8'),
  );
  bundlerOkamPkg.dependencies['@okamjs/okam'] = `${newVersion}`;
  fs.writeFileSync(
    bundlerOkamPkgPath,
    JSON.stringify(bundlerOkamPkg, null, 2) + '\n',
  );

  // build macOs *.node
  await $`rm ./crates/node/*.node`;
  await $`pnpm --filter @okamjs/okam run build:mac:x86`;
  await $`pnpm --filter @okamjs/okam run build:mac:aarch`;
  await $`strip -x ./crates/node/*.node`;

  await $`sed -i '' -e  '/*.node/d' ./crates/node/.npmignore`;

  // --no-git-checks because .npmignore modified
  // --ignore-scripts because we don't publish optional pkg
  await $`pnpm --filter @okamjs/okam --no-git-checks publish --ignore-scripts --tag ${tag} --access public`;
  await $`tnpm sync @okamjs/okam`;

  await $`git checkout crates/node/.npmignore`;

  // commit
  console.log('Commit');
  await $`git add ./`;
  // add (m) to commit message to indicate manually release and prevent release in CI
  await $`git commit -m "Release(m) ${newVersion}"`;

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
