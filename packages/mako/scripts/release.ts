import 'zx/globals';
import {
  ensureGitStatus,
  loadPkg,
  pushToGit,
  queryNewVersion,
  rootPkgPath,
  setNewVersionToBundlerOkam,
} from './utils';

(async () => {
  if (argv.build) {
    await build();
  } else {
    await run();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

async function run() {
  await ensureGitStatus();

  // check docker status
  console.log('Check docker status');
  await $`docker ps`;

  const nodePkgPath = rootPkgPath();
  const nodePkg = loadPkg(nodePkgPath);
  const { newVersion, tag, branch } = await queryNewVersion(nodePkg);
  nodePkg.version = newVersion;
  fs.writeFileSync(nodePkgPath, JSON.stringify(nodePkg, null, 2) + '\n');

  await build();

  await $`npm publish --tag ${tag} --access public`;

  setNewVersionToBundlerOkam(nodePkg.version);

  await pushToGit(nodePkg, branch);
}

async function build() {
  // clean
  await $`rm -rf ./*.node`;
  await $`find ./npm -name '*.node' | xargs rm -f`;
  await $`rm -rf ./dist`;

  // build linux *.node
  console.log('linux building started...');
  const start = Date.now();
  const cargoRoot = path.join(__dirname, '../../..');
  // clean sailfish
  // since its lock files may cause build error
  await $`rm -rf ${cargoRoot}/target/release/build/sailfish*`;
  await build_linux_binding();
  await $`pnpm run format`;
  const duration = (Date.now() - start) / 1000;
  console.log(`linux building done ${duration}s`);

  // build macos *.node
  await $`cargo build --lib -r --target x86_64-apple-darwin`;
  await $`pnpm run build:mac:x86`;
  await $`cargo build --lib -r  --target aarch64-apple-darwin`;
  await $`pnpm run build:mac:aarch`;
  await $`strip -x ./okam.darwin-*.node`;

  // build src
  await $`pnpm run src:build`;
  await $`pnpm run format`;

  // move artifacts to npm
  await $`pnpm run artifacts:local`;
}

async function build_linux_binding() {
  const isArm = process.arch === 'arm64';
  const cargoBase = path.join(
    process.env['CARGO_HOME'] || process.env['HOME']!,
    '.cargo',
  );
  const cargoMapOption = (p) => [
    '-v',
    `${path.join(cargoBase, p)}:${path.join('/usr/local/cargo', p)}`,
  ];
  const rustupRoot = path.join(os.homedir(), '.rustup');
  const makoRoot = path.join(__dirname, '../../..');
  const volumeOptions = [
    ...cargoMapOption('config'),
    ...cargoMapOption('git/db'),
    ...cargoMapOption('registry/cache'),
    ...cargoMapOption('registry/index'),
    ...[`-v`, `${makoRoot}:/build`],
    ...[`-v`, `${rustupRoot}:/usr/local/rustup`],
    ...[`-w`, `/build`],
  ];
  const containerCMD = [
    'cargo build -r --lib --target x86_64-unknown-linux-gnu',
    'cd packages/mako',
    'npm run build:linux:x86',
    'strip okam.linux*.node',
  ].join('&&');
  const envOptions: string[] = [];
  if (process.env['RUSTUP_DIST_SERVER']) {
    envOptions.push(
      ...['-e', `RUSTUP_DIST_SERVER=${process.env['RUSTUP_DIST_SERVER']}`],
    );
  }
  if (process.env[`RUSTUP_UPDATE_ROOT`]) {
    envOptions.push(
      ...['-e', `RUSTUP_UPDATE_ROOT=${process.env[`RUSTUP_UPDATE_ROOT`]}`],
    );
  }
  const options = ['--rm', ...volumeOptions, ...envOptions];
  if (isArm) {
    options.push(...['--platform', 'linux/amd64']);
  }
  const image = 'ghcr.io/napi-rs/napi-rs/nodejs-rust:lts-debian';
  await $`docker run ${options} ${image} bash -c ${containerCMD}`;
}
