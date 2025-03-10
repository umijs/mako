import url from 'url';
import { type Options } from 'sass';
import { BuildParams } from '../../';
import { RunLoadersOptions } from '../../runLoaders';

export class SassPlugin {
  name: string;
  params: BuildParams & { resolveAlias: Record<string, string> };
  sassOptions: Options<'async'>;
  extOpts: RunLoadersOptions;

  constructor(params: BuildParams & { resolveAlias: Record<string, string> }) {
    this.name = 'sass';
    this.params = params;
    this.extOpts = {
      alias: params.resolveAlias,
      root: params.root,
    };
    this.sassOptions = params.config?.sass || {};
  }

  load = async (filePath: string) => {
    let filename = '';
    try {
      filename = decodeURIComponent(url.parse(filePath).pathname || '');
    } catch (e) {
      return;
    }

    if (!filename?.endsWith('.scss')) {
      return;
    }

    const { render } = require('./render');
    return render({ filename, opts: this.sassOptions, extOpts: this.extOpts });
  };

  generateEnd = () => {};
}
