const { fork } = require('child_process');
const path = require('path');

class ForkTsChecker {
  constructor(projectRoot) {
    this.projectRoot = projectRoot;
  }

  runTypeCheckInChildProcess() {
    const workerScript = path.join(__dirname, 'child_process_fork.js');
    const child = fork(workerScript, [this.projectRoot], {
      stdio: 'inherit',
    });

    child.on('exit', (code) => {
      if (code === 1) {
        console.log('Type checking completed.');
      } else {
        console.error('Type checking failed.');
      }
    });
  }
}

module.exports = { ForkTsChecker };
