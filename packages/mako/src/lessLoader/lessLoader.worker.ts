import fs from 'fs';
import workerpool from 'workerpool';
import { LessLoaderOpts } from '.';

const ResolvePlugin = require('less-plugin-resolve');

const lessLoader = {
  render: async function (
    filePath: string,
    opts: LessLoaderOpts,
  ): Promise<string> {
    const { alias, modifyVars, math, sourceMap } = opts;
    const less = require('less');
    const input = fs.readFileSync(filePath, 'utf-8');
    const resolvePlugin = new ResolvePlugin({
      aliases: alias,
    });

    const result = await less.render(input, {
      filename: filePath,
      javascriptEnabled: true,
      math,
      plugins: [resolvePlugin],
      modifyVars,
      sourceMap,
      rewriteUrls: 'all',
    });
    return result.css;
  },
};

workerpool.worker(lessLoader);
