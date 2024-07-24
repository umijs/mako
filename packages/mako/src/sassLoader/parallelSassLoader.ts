import path from 'path';
import { Piscina } from 'piscina';
import { type Options } from 'sass';

export const createParallelLoader = () =>
  new Piscina<
    { filename: string; opts: Omit<Options<'async'>, 'functions'> },
    { content: string; type: 'css' }
  >({
    filename: path.resolve(__dirname + '/render.js'),
    idleTimeout: 30000,
    recordTiming: false,
    useAtomics: false,
  });
