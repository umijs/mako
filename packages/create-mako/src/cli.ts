import fs from 'fs';
import path, { resolve } from 'path';
import { globSync } from 'glob';
import type { QuestionCollection } from 'inquirer';
import yargs from 'yargs-parser';

const args = yargs(process.argv.slice(2));
const baseTemplatesPath = path.join(__dirname, '../templates');

type InitOptions = {
  projectName: string;
  template: string;
};

async function init({ projectName, template }: InitOptions) {
  let templatePath = path.join(baseTemplatesPath, template);
  if (!fs.existsSync(templatePath)) {
    console.error(`Template "${template}" does not exist.`);
    process.exit(1);
  }
  let files = globSync('**/*', { cwd: templatePath, nodir: true });

  // Use the project name entered by the user as the target folder name.
  let cwd = path.resolve(process.cwd(), projectName);

  // Ensure the target directory exists; if it does not, create it.
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, { recursive: true });
  }

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

type InitQuestion = {
  name: string;
  template: string;
};

async function checkEmptyDir(name: string) {
  const inquirer = (await import('inquirer')).default;
  const cwd = process.cwd();
  const exist = fs.existsSync(resolve(cwd, name));
  if (exist && fs.readdirSync(resolve(cwd, name)).length > 0) {
    const answersContinue = await inquirer.prompt([
      {
        type: 'confirm',
        name: 'continue',
        message:
          'The current directory is not empty. Do you want to continue creating the project here?',
        default: false,
      },
    ]);

    if (!answersContinue.continue) {
      process.exit(1);
    }
  }
}

async function main() {
  const inquirer = (await import('inquirer')).default;

  let name: string = args._[0] as string;
  let { template } = args;
  let questions: QuestionCollection[] = [];
  if (!name) {
    let answers = await inquirer.prompt<InitQuestion>([
      {
        type: 'input',
        name: 'name',
        message: 'Project name:',
        default: 'mako-project',
      },
    ]);
    name = answers.name;
  }
  await checkEmptyDir(name);
  if (!template) {
    const templates = globSync('**/', {
      cwd: baseTemplatesPath,
      maxDepth: 1,
    }).filter((dir) => dir !== '.');
    questions.push({
      type: 'list',
      name: 'template',
      message: 'Select a template:',
      choices: templates,
      default: 'react',
    });
  }
  if (questions.length > 0) {
    let answers = await inquirer.prompt<InitQuestion>(questions);
    template = template || answers.template;
  }
  return init({ projectName: String(name), template });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
