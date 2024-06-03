import 'zx/globals';

(async () => {
  // pnpm install to update lockfile
  console.log('Pnpm install');
  await $`pnpm install`;

  // build
  console.log('Build');
  await $`pnpm --filter create-mako build`;

  // update package.json to use the latest version of @umijs/mako
  console.log('Update package.json');
  const templatePkgPath = path.join(
    __dirname,
    '../packages/create-mako/templates/react/package.json',
  );
  const makoPkgPath = path.join(__dirname, '../packages/mako/package.json');
  const makoVersion = JSON.parse(fs.readFileSync(makoPkgPath, 'utf-8')).version;
  const templatePkg = JSON.parse(fs.readFileSync(templatePkgPath, 'utf-8'));
  templatePkg.devDependencies['@umijs/mako'] = `^${makoVersion}`;
  fs.writeFileSync(
    templatePkgPath,
    JSON.stringify(templatePkg, null, 2) + '\n',
    'utf-8',
  );

  // bump version
  console.log('Bump version');
  const pkgDir = path.join(__dirname, '../packages/create-mako/');
  const pkgPath = path.join(pkgDir, 'package.json');
  await $`cd packages/create-mako && npm version patch`;
  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));

  // git commit
  console.log('Commit');
  await $`git add ./`;
  await $`git commit -n -m "release: create-mako@${pkg.version}"`;

  // npm publish
  console.log('Publish');
  const tag = getTag(pkg.version);
  await $`cd packages/create-mako && npm publish --tag ${tag}`;

  // git push
  console.log('Push');
  await $`git push origin master`;

  console.log('Done');
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
