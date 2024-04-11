import fs from 'fs';
import url from 'url';

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
  implementation?: any;
}

export function lessLoader(fn: Function | null, opts: LessLoaderOpts) {
  return async function (filePath: string) {
    let pathname = '';
    try {
      pathname = url.parse(filePath).pathname || '';
    } catch (e) {
      return;
    }
    if (pathname?.endsWith('.less')) {
      const { alias, modifyVars, math, sourceMap } = opts;
      if (opts.implementation) {
        const less = opts.implementation;
        const input = fs.readFileSync(pathname, 'utf-8');
        const resolvePlugin = new (require('less-plugin-resolve'))({
          aliases: alias,
        });
        const result = await less.render(input, {
          filename: pathname,
          javascriptEnabled: true,
          math,
          plugins: [resolvePlugin],
          modifyVars,
          sourceMap,
          rewriteUrls: 'all',
        });
        return { content: result.css, type: 'css' };
      } else {
        const less = require('./parallelLessLoader').less;
        return less(pathname, opts);
      }
    } else {
      // TODO: remove this
      fn && fn(filePath);
    }
  };
}
