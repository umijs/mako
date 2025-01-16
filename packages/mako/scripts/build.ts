import assert from 'assert';
import 'zx/globals';

(async () => {
  const cwd = process.cwd();

  console.log('[build] clean *.node and dist');
  await $`rm -rf ./*.node`;
  await $`find ./npm -name '*.node' | xargs rm -f`;
  await $`rm -rf ./dist`;

  if (argv['clean-only'] || argv.cleanOnly) {
    console.log('[build] exit since --clean-only is set');
    return;
  }

  const commitId = (await $`git rev-parse HEAD`).stdout.trim();
  const artifactsFile = path.join(cwd, `artifacts-${commitId}.zip`);
  assert(fs.existsSync(artifactsFile), `${artifactsFile} not found`);

  console.log('[build] build napi');
  await $`pnpm run napi:build`;
  console.log('[build] build src');
  await $`pnpm run src:build`;
  console.log('[build] format');
  await $`pnpm run format`;

  console.log('[build] unzip artifacts');
  await $`rm -rf ./*.node`;
  await $`unzip ${artifactsFile}`;
  console.log('[build] artifacts:local');
  await $`npm run artifacts:local`;
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
