import fs from 'fs';
import path from 'path';
import commander from 'commander';
import { globSync } from 'glob';
import packageJson from '../package.json';

async function init(projectName: string) {
  let templatePath = path.join(__dirname, '../templates/react');
  let files = globSync('**/*', { cwd: templatePath, nodir: true });
  let cwd = path.join(process.cwd(), projectName);

  let npmClient = (() => {
    let script = process.argv[1];
    if (script.includes('pnpm/')) {
      return 'pnpm';
    } else {
      return 'npm';
    }
  })();

  // Copy files
  for (let file of files) {
    let source = path.join(templatePath, file);
    let dest = path.join(cwd, file);
    console.log(`Creating ${file}`);
    let destDir = path.dirname(dest);
    if (!fs.existsSync(destDir)) {
      fs.mkdirSync(destDir, { recursive: true });
    }
    fs.copyFileSync(source, dest);
  }
  console.log();
  console.log('Done, Run following commands to start the project:');
  console.log();
  console.log(`  cd ${path.basename(cwd)}`);
  console.log(`  ${npmClient} install`);
  console.log(`  ${npmClient} run dev`);
  console.log(`  # Open http://localhost:3000`);
  console.log();
  console.log('Happy coding!');
}

async function main() {
  new commander.Command(packageJson.name)
    .version(packageJson.version)
    .argument('[project-directory]', 'Project directory')
    .usage(`[project-directory]`)
    .action(async (name: string) => {
      if (!name) {
        const inquirer = (await import('inquirer')).default;
        let answers = await inquirer.prompt([
          {
            type: 'input',
            name: 'name',
            message: 'Project name:',
            default: 'mako-project',
          },
        ]);
        name = answers.name;
      }
      init(name).catch((err) => {
        console.error(err);
        process.exit(1);
      });
    })
    .parse(process.argv);
}

main();
