import url from 'url';
import { compile, terminatePool } from './parallelLessLoader';

export interface LessLoaderOpts {
  alias: Record<string, string>;
  modifyVars: Record<string, string>;
  math:
    | 'always'
    | 'strict'
    | 'parens-division'
    | 'parens'
    | 'strict-legacy'
    | number
    | undefined;
  sourceMap: any;
}

function lessLoader(fn: Function | null, opts: LessLoaderOpts) {
  return async function (filePath: string) {
    let pathname = '';
    try {
      pathname = url.parse(filePath).pathname || '';
    } catch (e) {
      return;
    }
    if (pathname?.endsWith('.less')) {
      return compile(pathname, opts);
    } else {
      // TODO: remove this
      fn && fn(filePath);
    }
  };
}

lessLoader.terminate = () => {
  terminatePool();
};

export { lessLoader };
