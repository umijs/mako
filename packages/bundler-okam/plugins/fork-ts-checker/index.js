const { fork } = require('child_process');
const path = require('path');
const { TypeChecker } = require('./ts-checker');

class ForkTsChecker {
  constructor(projectRoot) {
    this.projectRoot = projectRoot;
  }
  async runTypeCheck() {
    const typeChecker = new TypeChecker(projectRoot);
    await typeChecker.check();
  }

  runTypeCheckInChildProcess() {
    const workerScript = path.join(__dirname, 'child_process_fork.js');
    const child = fork(workerScript, [projectRoot], {
      stdio: 'inherit',
    });

    child.on('exit', (code) => {
      if (code === 0) {
        console.log('Type checking completed successfully.');
      } else {
        console.error('Type checking failed.');
      }
    });
  }
}

module.exports = { ForkTsChecker };
