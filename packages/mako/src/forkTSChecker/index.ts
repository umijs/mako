import { fork } from 'child_process';
import path from 'path';

interface ForkTSCheckerOpts {
  root: string;
  watch: boolean;
}

export class ForkTSChecker {
  #opts: ForkTSCheckerOpts;
  constructor(opts: ForkTSCheckerOpts) {
    this.#opts = opts;
  }

  runTypeCheckInChildProcess() {
    const workerScript = path.join(__dirname, 'childProcessFork.js');
    const child = fork(workerScript, [this.#opts.root], {
      stdio: 'inherit',
    });
    child.on('exit', (code) => {
      if (code === 1) {
        console.error('Type checking failed.');
        if (!this.#opts.watch) {
          process.exit(1);
        }
      }
    });
  }
}
