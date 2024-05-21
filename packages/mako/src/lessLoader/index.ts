import url from 'url';
/**
 * When on platform linux ant node version is lower then 20,
 * the worker pool may exit unexpectedly, we need to disable it.
 * This may be is problem of nodejs, may related issue: https://github.com/nodejs/node/issues/51129.
 */
const DisableParallelLess =
  process.platform === 'linux' &&
  parseInt(process.versions.node.split('.')[0]) < 20;

export interface LessLoaderOpts {
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
  /**
   * A plugin can be a file path string, or a file path string with a params object.
   * Notice! The file path should be a resolved path like require.resolve("less-plugin-clean-css"),
   * and the params object must be a plain json. We will require the plugin file to get the plugin content.
   * If the params object been accepted, that means, the required content will be treated as a factory class of Less.Plugin,
   * we will create a plugin instance with the params object, or else, the required content will be treated as a plugin instance.
   * We do this because the less loader runs in a worker pool for speed, and a less plugin instance can't be passed to worker directly.
   */
  plugins?: (string | [string, Record<string, any>])[];
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
      return DisableParallelLess
        ? require('./render').render(pathname, opts)
        : require('./parallelLessLoader').render(pathname, opts);
    } else {
      // TODO: remove this
      fn && fn(filePath);
    }
  };
}

lessLoader.terminate = () => {
  !DisableParallelLess && require('./parallelLessLoader').terminatePool();
};

export { lessLoader };
