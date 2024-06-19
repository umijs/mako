import chalk from 'chalk';
import yParser from 'yargs-parser';
import { check } from './checker';

(async () => {
  let isWin = process.platform === 'win32';
  if (isWin) {
    console.error(
      'mako is not supported on Windows yet, please visit https://makojs.dev/ to subscribe for updates',
    );
    process.exit(1);
  }

  // use MAKO_CLI to identify if it's running in mako cli standalone
  // so that we can print extra information
  process.env.MAKO_CLI = '1';
  console.log();
  console.log(chalk.bold(`Mako v${require('../package.json').version}`));
  console.log();

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
      check(root);
      await require('./').build({
        root,
        config: {
          mode: argv.mode || 'development',
        },
        plugins: [],
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
