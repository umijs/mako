import os from 'os';
import { RunLoaderResult } from 'loader-runner';
import { Piscina } from 'piscina';
import { RunLoadersOptions } from '.';

export function createParallelLoader<T>(renderPath: string) {
  return new Piscina<
    {
      filename: string;
      content?: string;
      opts?: T;
      extOpts: RunLoadersOptions;
    },
    RunLoaderResult & { missingDependencies: string[] }
  >({
    filename: renderPath,
    idleTimeout: 30000,
    recordTiming: false,
    useAtomics: false,
    maxThreads:
      // when cpus is less than "4", the worker pool may lead to panic on linux
      os.platform() === 'linux' ? Math.max(os.cpus().length - 3, 1) : undefined,
  });
}
