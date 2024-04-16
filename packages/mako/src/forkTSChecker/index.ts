import { fork } from 'child_process';
import path from 'path';

export class ForkTsChecker {
  #projectRoot: string;
  constructor(projectRoot: string) {
    this.#projectRoot = projectRoot;
  }

  runTypeCheckInChildProcess() {
    const workerScript = path.join(__dirname, 'childProcessFork.js');
    const child = fork(workerScript, [this.#projectRoot], {
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
