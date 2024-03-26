const { TypeChecker } = require('./ts-checker');

const projectRoot = process.argv[2];

async function runTypeCheck() {
  const typeChecker = new TypeChecker(projectRoot);
  return await typeChecker.check();
}

runTypeCheck()
  .then(() => {
    process.exit(1);
  })
  .catch((error) => {
    console.error(error);
    process.exit(0);
  });
