import fs from 'fs';
import path from 'path';
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

type InitQuestion = {
  name: string;
  template: string;
};
async function main() {
  let name = args._[0];
  let { template } = args;
  let questions: QuestionCollection[] = [];
  if (!name) {
    questions.push({
      type: 'input',
      name: 'name',
      message: 'Project name:',
      default: 'mako-project',
    });
  }
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
    const inquirer = (await import('inquirer')).default;
    let answers = await inquirer.prompt<InitQuestion>(questions);
    name = name || answers.name;
    template = template || answers.template;
  }
  return init({ projectName: String(name), template });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
