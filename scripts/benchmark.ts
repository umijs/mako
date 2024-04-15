import assert from 'assert';
import 'zx/globals';

(async () => {
  const shouldBuild = argv.build !== false;
  const baseline = argv.baseline || 'master';
  const casePath =
    argv.case ||
    (argv.multiChunks ? './tmp/three10x/multiChunks' : './tmp/three10x');

  const originBranch = (await $`git rev-parse --abbrev-ref HEAD`).stdout.trim();
  const isGitClean = (await $`git status --porcelain`).stdout.trim() === '';
  if (!isGitClean) {
    await $`git stash --include-untracked`;
  }
  await $`git checkout ${baseline}`;

  const baselineHash = (await $`git rev-parse --short HEAD`).stdout.trim();
  const baselineMakoPath = `./tmp/mako-${baselineHash}`;
  if (!fs.existsSync(path.join(__dirname, `../tmp/mako-${baselineHash}`))) {
    if (shouldBuild) {
      await $`cargo build --release`;
      await $`cp target/release/mako ${baselineMakoPath}`;
    } else {
      console.log(`Since --no-build is set, build for baseline is skipped.`);
    }
  }

  await $`git checkout ${originBranch}`;
  if (!isGitClean) {
    await $`git stash pop`;
  }

  // build latest mako
  if (shouldBuild) {
    await $`cargo build --release`;
  } else {
    console.log(`Since --no-build is set, build for current mako is skipped.`);
  }

  let currentMakoPath = './target/release/mako';
  if (isGitClean) {
    const currentHash = (await $`git rev-parse --short HEAD`).stdout.trim();
    const makoCurrentName = `mako-${currentHash}`;
    await $`cp target/release/mako ./tmp/${makoCurrentName}`;
    currentMakoPath = `./tmp/${makoCurrentName}`;
  }

  assert(
    currentMakoPath !== baselineMakoPath,
    'currentMakoPath should not be equal to baselineMakoPath',
  );
  console.log(path.join(__dirname, '..', currentMakoPath));
  assert(
    fs.existsSync(path.join(__dirname, '..', currentMakoPath)),
    `current mako binary should exist: ${currentMakoPath}`,
  );
  console.log(path.join(__dirname, '..', baselineMakoPath), baselineMakoPath);
  assert(
    fs.existsSync(path.join(__dirname, '..', baselineMakoPath)),
    `baseline mako binary should exist: ${baselineMakoPath}`,
  );

  // run benchmark
  const warmup = argv.warmup || 3;
  const runs = argv.runs || 10;
  await $`hyperfine --warmup ${warmup} --runs ${runs} "${currentMakoPath} ${casePath} --mode production" "${baselineMakoPath} ${casePath} --mode production"`;
})();
