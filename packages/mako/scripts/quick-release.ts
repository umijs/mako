import assert from 'assert';
import 'zx/globals';

(async () => {
  if (argv.build) {
    await build();
  } else {
    await run();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

async function run() {
  // check git status
  console.log('Check git status');
  const status = (await $`git status --porcelain`).stdout.trim();
  if (status) {
    // throw new Error('Please commit all changes before release');
  }

  // check git remote update
  console.log('check git remote update');
  await $`git fetch`;
  const gitStatus = (await $`git status --short --branch`).stdout.trim();
  assert(!gitStatus.includes('behind'), `git status is behind remote`);

  const commitId = (await $`git rev-parse HEAD`).stdout.trim();
  const artficatsFile = `artifacts-${commitId}.tar`;

  const hasArtifacts = fs.existsSync(path.join(process.cwd(), artficatsFile));

  assert(hasArtifacts, `${artficatsFile} not found in cwd`);

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

  console.log('Check branch');
  const branch = (await $`git branch --show-current`).stdout.trim();
  if (tag === 'latest') {
    if (branch !== 'master') {
      throw new Error('publishing latest tag needs to be in master branch');
    }
  }

  // confirm
  console.log(`${nodePkg.name}@${newVersion} will be published`);
  const willContinue = ((await question('Continue? y/[n]')) || 'n').trim();
  if (willContinue !== 'y') {
    console.log('Abort!');
    process.exit(1);
  }

  // update version to package.json
  nodePkg.version = newVersion;
  fs.writeFileSync(nodePkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  await build();

  await $`rm -rf *.node`;
  await $`tar -vxf ${artficatsFile}`;
  await $`npm run artifacts:local`;

  // publish
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

  await $`git commit -an -m "release: ${nodePkg.name}@${newVersion}"`;
  // tag
  console.log('Tag');
  await $`git tag v${newVersion}`;

  // push
  console.log('Push');
  await $`git push origin ${branch} --tags`;
}

async function build() {
  // clean
  await $`rm -rf ./*.node`;
  await $`find ./npm -name '*.node' | xargs rm -f`;
  await $`rm -rf ./dist`;

  await $`pnpm run build`;
  await $`pnpm run src:build`;
  await $`pnpm run format`;
}
