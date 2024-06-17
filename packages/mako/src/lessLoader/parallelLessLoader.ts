import path from 'path';
import { Piscina } from 'piscina';
import { LessLoaderOpts } from '.';

const threadPool = new Piscina<
  { filename: string; opts: LessLoaderOpts },
  { content: string; type: 'css' }
>({
  filename: path.resolve(__dirname + '/render.js'),
  idleTimeout: 30000,
  recordTiming: false,
  useAtomics: false,
});

export async function render(
  filename: string,
  opts: LessLoaderOpts,
): Promise<{ content: string; type: 'css' }> {
  return await threadPool.run({ filename, opts });
}

export function terminatePool() {
  threadPool.close();
}
