import 'zx/globals';

(async () => {
  // pnpm install to update lockfile
  console.log('pnpm install');
  await $`pnpm install`;

  // bump version to sync with @okamjs/okam
  console.log('Bump version');
  const bundlerOkamPkgPath = path.join(
    __dirname,
    '../packages/bundler-okam/package.json',
  );
  const bundlerOkamPkg = JSON.parse(
    fs.readFileSync(bundlerOkamPkgPath, 'utf-8'),
  );
  bundlerOkamPkg.version = bundlerOkamPkg.dependencies['@okamjs/okam'];
  fs.writeFileSync(
    bundlerOkamPkgPath,
    JSON.stringify(bundlerOkamPkg, null, 2) + '\n',
  );

  // git commit
  console.log('Commit');
  await $`git add ./`;
  await $`git commit -m "chore: bundler-okam@${bundlerOkamPkg.version}"`;

  console.log('Done');
  console.log('Please run the following command to publish:');
  console.log('cd packages/bundler-okam && tnpm publish');
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
