import path from 'path';
import workerpool from 'workerpool';
import { LessLoaderOpts } from './lessLoader';

let pool: workerpool.Pool | undefined = undefined;

export function createPool() {
  if (!pool) {
    pool = workerpool.pool(path.resolve(__dirname + '/lessLoader.worker.js'));
  }
}

export function destoryPool() {
  pool?.terminate();
  pool = undefined;
}

export async function less(
  filePath: string,
  opts: Omit<LessLoaderOpts, 'implementation'>,
) {
  createPool();
  return pool!.exec<
    (
      filePath: string,
      opts: Omit<LessLoaderOpts, 'implementation'>,
    ) => Promise<{
      content: any;
      type: string;
    }>
  >('compileLess', [filePath, opts]);
}
