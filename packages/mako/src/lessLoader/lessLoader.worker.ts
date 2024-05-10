import fs from 'fs';
import less from 'less';
import workerpool from 'workerpool';
import { LessLoaderOpts } from '.';

const lessLoader = {
  render: async function (
    filePath: string,
    opts: LessLoaderOpts,
  ): Promise<string> {
    const { modifyVars, math, sourceMap, pluginsForMako } = opts;
    const input = fs.readFileSync(filePath, 'utf-8');

    const plugins: Less.Plugin[] | undefined = pluginsForMako?.map((p) => {
      if (Array.isArray(p)) {
        const pluginClass = require(p[0]);
        return new pluginClass(p[1]);
      } else {
        return require(p);
      }
    });

    const result = await less
      .render(input, {
        filename: filePath,
        javascriptEnabled: true,
        math,
        plugins,
        modifyVars,
        sourceMap,
        rewriteUrls: 'all',
      } as unknown as Less.Options)
      .catch((err) => {
        throw new Error(err.toString());
      });

    return result.css;
  },
};

workerpool.worker(lessLoader);
