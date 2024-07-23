import url from 'url';
import { type Options } from 'sass';
import { createParallelLoader } from './parallelSassLoader';

function sassLoader(fn: Function | null, opts: Options<'async'>) {
  let parallelSassLoader: ReturnType<typeof createParallelLoader> | undefined;
  return {
    render: async (filePath: string) => {
      let filename = '';
      try {
        filename = decodeURIComponent(url.parse(filePath).pathname || '');
      } catch (e) {
        return;
      }
      if (filename?.endsWith('.scss')) {
        parallelSassLoader ||= createParallelLoader();
        return await parallelSassLoader.run({ filename, opts });
      } else {
        // TODO: remove this
        fn && fn(filePath);
      }
    },
    terminate: () => {
      parallelSassLoader?.destroy();
      parallelSassLoader = undefined;
    },
  };
}

export { sassLoader };
