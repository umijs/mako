import assert from 'assert';
import 'zx/globals';

export async function ensureGitStatus() {
  console.log('Check git status');
  const status = (await $`git status --porcelain`).stdout.trim();
  if (status) {
    throw new Error('Please commit all changes before release');
  }

  console.log('check git remote update');
  await $`git fetch`;
  const gitStatus = (await $`git status --short --branch`).stdout.trim();
  assert(!gitStatus.includes('behind'), `git status is behind remote`);
}

export function rootPkgPath() {
  const nodePkgDir = path.resolve(__dirname, '..');
  return path.join(nodePkgDir, 'package.json');
}

export function loadPkg(nodePkgPath: string) {
  const nodePkg = JSON.parse(fs.readFileSync(nodePkgPath, 'utf-8'));
  return nodePkg;
}

export async function queryNewVersion(nodePkg: any) {
  console.log('Bump version');
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

  return { newVersion, tag, branch } as const;
}

export function setNewVersionToBundlerOkam(newVersion: string) {
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
}

export async function pushToGit(
  nodePkg: { name: string; version: string },
  branch: string,
) {
  await $`git commit -an -m "release: ${nodePkg.name}@${nodePkg.version}"`;

  console.log('Tag');
  await $`git tag v${nodePkg.version}`;

  console.log('Push');
  await $`git push origin ${branch} --tags`;
}
