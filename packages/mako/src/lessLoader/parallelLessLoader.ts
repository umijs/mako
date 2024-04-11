import path from 'path';
import workerpool from 'workerpool';
import { LessLoaderOpts } from '.';

let pool: workerpool.Pool | undefined = undefined;

function createPool() {
  if (!pool) {
    pool = workerpool.pool(path.resolve(__dirname + '/lessLoader.worker.js'));
  }
}

export function terminate() {
  pool?.terminate();
  pool = undefined;
}

export async function compile(
  filePath: string,
  opts: LessLoaderOpts,
): Promise<{ content: string; type: string }> {
  createPool();

  return pool!.exec('render', [filePath, opts]);
}
