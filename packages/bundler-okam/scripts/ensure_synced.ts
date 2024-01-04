import 'zx/globals';

(async () => {
  const makoVersion =
    require('../package.json')['dependencies']['@okamjs/okam'];

  await retry(3, () => {
    return (async () => {
      await $`tnpm sync @okamjs/okam`;
      await $`tnpm info @okamjs/okam@${makoVersion}`;
    })();
  });
})().catch((err) => {
  console.error(err);
  console.error('sync @okamjs/okam failed!');
  process.exit(1);
});
