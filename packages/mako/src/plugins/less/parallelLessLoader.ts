import path from 'path';
import { RunLoaderResult } from 'loader-runner';
import { Piscina } from 'piscina';
import { LessLoaderOpts } from '.';
import { RunLoadersOptions } from '../../runLoaders';

export const createParallelLoader = () =>
  new Piscina<
    { filename: string; opts: LessLoaderOpts; extOpts: RunLoadersOptions },
    RunLoaderResult & { missingDependencies: string[] }
  >({
    filename: path.resolve(__dirname + '/render.js'),
    idleTimeout: 30000,
    recordTiming: false,
    useAtomics: false,
  });
