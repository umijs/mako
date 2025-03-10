import path from 'path';
import { Piscina } from 'piscina';
import { LessLoaderOpts } from '.';
import { RunLoadersOptions } from '../../runLoaders';

export const createParallelLoader = () =>
  new Piscina<
    { filename: string; opts: LessLoaderOpts; extOpts: RunLoadersOptions },
    { content: string; type: 'css' }
  >({
    filename: path.resolve(__dirname + '/render.js'),
    idleTimeout: 30000,
    recordTiming: false,
    useAtomics: false,
  });
