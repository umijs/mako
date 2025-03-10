import url from 'url';
import { type Options } from 'sass';
import { RunLoadersOptions } from '../runLoaders';

function sassLoader(
  fn: Function | null,
  opts: Options<'async'>,
  extOpts: RunLoadersOptions,
) {
  return {
    render: async (filePath: string) => {
      let filename = '';
      try {
        filename = decodeURIComponent(url.parse(filePath).pathname || '');
      } catch (e) {
        return;
      }
      if (filename?.endsWith('.scss')) {
        const { render } = require('./render');
        return render({ filename, opts, extOpts });
      } else {
        // TODO: remove this
        fn && fn(filePath);
      }
    },
    terminate: () => {},
  };
}

export { sassLoader };
