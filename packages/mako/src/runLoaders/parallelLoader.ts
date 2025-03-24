import { RunLoaderResult } from 'loader-runner';
import { Piscina } from 'piscina';
import { RunLoadersOptions } from '.';

export function createParallelLoader<T>(renderPath: string) {
  return new Piscina<
    {
      filename: string;
      opts: T;
      extOpts: RunLoadersOptions;
      postcss?: boolean;
    },
    RunLoaderResult & { missingDependencies: string[] }
  >({
    filename: renderPath,
    idleTimeout: 30000,
    recordTiming: false,
    useAtomics: false,
  });
}
