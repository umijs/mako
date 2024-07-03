import fs from 'fs';
import path from 'path';
import { globSync } from 'glob';
import yargs from 'yargs-parser';

const args = yargs(process.argv.slice(2));

async function init(projectName: string) {
  let templatePath = path.join(__dirname, '../templates/react');
  let files = globSync('**/*', { cwd: templatePath, nodir: true });

  // 使用用户输入的项目名称作为目标文件夹名
  let cwd = path.resolve(process.cwd(), projectName);

  // 确保目标目录存在，如果不存在则创建
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

async function main() {
  const inquirer = (await import('inquirer')).default;

  // 提示用户是否继续创建项目
  const answersContinue = await inquirer.prompt([
    {
      type: 'confirm',
      name: 'continue',
      message: '当前目录不为空，是否继续在此创建项目？',
      default: false,
    },
  ]);

  if (!answersContinue.continue) {
    return;
  }

  let name = args._[0];
  if (!name) {
    let answers = await inquirer.prompt([
      {
        type: 'input',
        name: 'name',
        message: 'Project name：',
        default: 'mako-project',
      },
    ]);
    name = answers.name;
  }

  return init(String(name));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
