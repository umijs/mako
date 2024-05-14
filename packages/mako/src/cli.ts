import yParser from 'yargs-parser';

(async () => {
  let argv = yParser(process.argv.slice(2));
  let command = argv._[0];
  switch (command) {
    case 'build':
      if (argv.help || argv.h) {
        showBuildHelp();
        break;
      }
      let watch = argv.watch || argv.w || false;
      let root = argv.root || process.cwd();
      await require('./').build({
        root,
        config: {
          mode: argv.mode || 'development',
        },
        hooks: {},
        watch,
      });
      break;
    case undefined:
      if (argv.version || argv.v) {
        console.log(require('../package.json').version);
        break;
      } else {
        showHelp();
        break;
      }
    default:
      console.error(`Unknown command: ${command}`);
      process.exit(1);
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});

function showHelp() {
  console.log(`Usage: mako <command> [options]`);
  console.log(``);
  console.log(`Commands:`);
  console.log(`  build`);
  console.log(``);
  console.log(`Options:`);
  console.log(`  --help,-h`);
  console.log(`  --version,-v`);
  console.log(``);
  console.log(`Examples:`);
  console.log(`  mako build`);
  console.log(`  mako -v`);
}

function showBuildHelp() {
  console.log(`Usage: mako build [options]`);
  console.log(``);
  console.log(`Options:`);
  console.log(`  --help,-h`);
  console.log(`  --root`);
  console.log(`  --watch,-w`);
  console.log(``);
  console.log(`Examples:`);
  console.log(`  mako build`);
  console.log(`  mako build --watch`);
  console.log(`  mako build --root ./src`);
}
