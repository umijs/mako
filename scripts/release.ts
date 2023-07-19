import 'zx/globals';
import { parse } from 'semver';

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
  const version = nodePkg.version;
  const parsedVersion = parse(version);
  if (!parsedVersion) {
    throw new Error(`Invalid version: ${version}`);
  }
  parsedVersion.patch += 1;
  const newVersion = parsedVersion.format();
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

  // commit
  console.log('Commit');
  await $`git add ./`;
  await $`git commit -m "Release ${newVersion}"`;

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
