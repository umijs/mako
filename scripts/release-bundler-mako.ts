import 'zx/globals';

(async () => {
  // pnpm install to update lockfile
  console.log('pnpm install');
  await $`pnpm install`;

  // bump version to sync with @okamjs/okam
  console.log('Bump version');
  const bundlerMakoPkgPath = path.join(
    __dirname,
    '../packages/bundler-mako/package.json',
  );
  const bundlerMakoPkg = JSON.parse(
    fs.readFileSync(bundlerMakoPkgPath, 'utf-8'),
  );
  bundlerMakoPkg.version = bundlerMakoPkg.dependencies['@umijs/mako'];
  fs.writeFileSync(
    bundlerMakoPkgPath,
    JSON.stringify(bundlerMakoPkg, null, 2) + '\n',
  );

  // git commit
  console.log('Commit');
  await $`git add ./`;
  await $`git commit -n -m "chore: bundler-mako@${bundlerMakoPkg.version}"`;

  // npm publish
  console.log('Publish');
  let tag = 'latest';
  const newVersion = bundlerMakoPkg.version;
  if (
    newVersion.includes('-alpha.') ||
    newVersion.includes('-beta.') ||
    newVersion.includes('-rc.')
  )
    tag = 'next';
  if (newVersion.includes('-canary.')) tag = 'canary';
  if (newVersion.includes('-dev.')) tag = 'dev';
  await $`cd packages/bundler-mako && npm publish --tag ${tag}`;

  // git push
  console.log('Push');
  await $`git push origin master`;

  console.log('Done');
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
