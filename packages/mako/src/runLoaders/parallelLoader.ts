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
      // strange panic on docker linux
      os.platform() === 'linux' ? (os.cpus().length > 4 ? 2 : 1) : undefined,
  });
}
