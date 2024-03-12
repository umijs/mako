import 'zx/globals';

(async () => {
  const baseline = argv.baseline || 'master';
  const casePath = argv.case || './tmp/three10x';

  const currentBranch = (await $`git rev-parse --abbrev-ref HEAD`).stdout.trim();
  const isGitClean = (await $`git status --porcelain`).stdout.trim() === '';
  const isBaselineMaster = baseline === 'master';
  const makoBaselineName = isBaselineMaster ? 'mako-master' : `mako-${baseline}`;
  const makoBaselineRelativePath = path.join(__dirname, `../tmp/${makoBaselineName}`);
  const isBaselineMakoExists = fs.existsSync(makoBaselineRelativePath);
  console.log('currentBranch', currentBranch);
  console.log('isGitClean', isGitClean);
  console.log('isBaselineMaster', isBaselineMaster);
  console.log('makoBaselineName', makoBaselineName);
  console.log('makoBaselineRelativePath', makoBaselineRelativePath);
  console.log('isBaselineMakoExists', isBaselineMakoExists);
  console.log('argv', argv);

  async function buildBaselineMako() {
    if (!isGitClean) {
      await $`git stash`;
    }
    await $`git checkout ${baseline}`;
    await $`cargo build --release`;
    await $`cp target/release/mako ./tmp/${makoBaselineName}`;
    await $`git checkout ${currentBranch}`;
    if (!isGitClean) {
      await $`git stash pop`;
    }
  }

  // build baseline mako
  if (isBaselineMaster) {
    // master may change, so always build except --skip-baseline-build is supplied
    if (!argv.skipBaselineBuild) {
      await buildBaselineMako();
    }
  } else {
    if (!isBaselineMakoExists || argv.force) {
      await buildBaselineMako();
    }
  }

  const isBaselineMakoExistsAfterBuild = fs.existsSync(makoBaselineRelativePath);
  if (!isBaselineMakoExistsAfterBuild) {
    throw new Error('Baseline mako not found');
  }

  // build latest mako
  await $`cargo build --release`;

  // run benchmark
  await $`hyperfine --warmup 1 --runs 3 "./target/release/mako ${casePath} --mode production" "./tmp/${makoBaselineName} ${casePath} --mode production"`;
})();
