import url from 'url';
import { type Options } from 'sass';

function sassLoader(fn: Function | null, opts: Options<'async'>) {
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
        return render({ filename, opts });
      } else {
        // TODO: remove this
        fn && fn(filePath);
      }
    },
    terminate: () => {},
  };
}

export { sassLoader };
