import assert from 'assert';
import 'zx/globals';
import {
  ensureGitStatus,
  loadPkg,
  pushToGit,
  queryNewVersion,
  rootPkgPath,
  setNewVersionToBundlerMako,
} from './utils';

(async () => {
  await run();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

async function run() {
  await ensureGitStatus();

  const commitId = (await $`git rev-parse HEAD`).stdout.trim();
  const artifactsFile = `artifacts-${commitId}.zip`;
  const hasArtifacts = fs.existsSync(path.join(process.cwd(), artifactsFile));
  assert(hasArtifacts, `${artifactsFile} not found in cwd`);

  const nodePkgPath = rootPkgPath();
  const nodePkg = loadPkg(nodePkgPath);
  const { newVersion, tag, branch } = await queryNewVersion(nodePkg);

  nodePkg.version = newVersion;
  fs.writeFileSync(nodePkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  await build();
  await artifacts(artifactsFile);

  await $`npm publish --tag ${tag} --access public`;

  setNewVersionToBundlerMako(nodePkg.version);

  await pushToGit(nodePkg, branch);
}

async function build() {
  await $`rm -rf ./*.node`;
  await $`find ./npm -name '*.node' | xargs rm -f`;
  await $`rm -rf ./dist`;

  await $`pnpm run build`;
  await $`pnpm run src:build`;
  await $`pnpm run format`;
}

async function artifacts(artifactsFile: string) {
  await $`rm -rf *.node`;
  await $`unzip ${artifactsFile}`;
  await $`npm run artifacts:local`;
}
