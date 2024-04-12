import assert from 'assert';
import 'zx/globals';
import {
  ensureGitStatusClean,
  loadPkg,
  pushToGit,
  queryNewVersion,
  rootPkgPath,
  setNewVersionToBundler,
} from './utils';

(async () => {
  await run();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

async function run() {
  await ensureGitStatusClean();

  const commitId = (await $`git rev-parse HEAD`).stdout.trim();
  const artifactsFile = `artifacts-${commitId}.tar`;
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

  setNewVersionToBundler(nodePkg);

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
  await $`tar -vxf ${artifactsFile}`;
  await $`npm run artifacts:local`;
}
