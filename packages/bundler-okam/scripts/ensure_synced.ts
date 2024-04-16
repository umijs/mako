import 'zx/globals';

(async () => {
  const makoVersion =
    require('../package.json')['dependencies']['@okamjs/okam'];

  await retry(3, () => {
    return (async () => {
      await $`tnpm sync @okamjs/okam`.quiet();
      const info =
        await $`tnpm info --json @okamjs/okam@${makoVersion}`.quiet();

      const optionDeps = JSON.parse(info.stdout)['optionalDependencies'];
      await Promise.all(
        Object.keys(optionDeps).map((key) => {
          return $`tnpm sync ${key} && tnpm info ${key}@${optionDeps[key]}`.quiet();
        }),
      );
    })();
  });

  console.log(chalk.bgGreen(chalk.white('SUCCEED')), chalk.green('synced'));
})().catch((err) => {
  console.error(err);
  console.error(
    chalk.bgRed(chalk.white('FAILED')),
    chalk.red('sync @okamjs/okam failed!'),
  );
  process.exit(1);
});
