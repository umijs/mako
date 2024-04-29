import 'zx/globals';

(async () => {
  // pnpm install to update lockfile
  console.log('Pnpm install');
  await $`pnpm install`;

  // build
  console.log('Build');
  await $`pnpm --filter @okamjs/rsc build`;

  // bump version to sync with @okamjs/okam
  console.log('Bump version');
  const pkgDir = path.join(__dirname, '../packages/rsc/');
  const pkgPath = path.join(pkgDir, 'package.json');
  await $`cd packages/rsc && npm version patch`;
  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));

  // git commit
  console.log('Commit');
  await $`git add ./`;
  await $`git commit -n -m "chore: rsc@${pkg.version}"`;

  // npm publish
  console.log('Publish');
  const tag = getTag(pkg.version);
  await $`cd packages/rsc && npm publish --tag ${tag}`;

  // git push
  console.log('Push');
  await $`git push origin master --tags`;
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

function getTag(newVersion: string) {
  let tag = 'latest';
  if (
    newVersion.includes('-alpha.') ||
    newVersion.includes('-beta.') ||
    newVersion.includes('-rc.')
  )
    tag = 'next';
  if (newVersion.includes('-canary.')) tag = 'canary';
  if (newVersion.includes('-dev.')) tag = 'dev';
  return tag;
}
