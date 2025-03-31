import path from 'path';
import url from 'url';
import { BuildParams } from '../../';
import * as binding from '../../../binding';
import { RunLoadersOptions, createParallelLoader } from '../../runLoaders';

export class PostcssPlugin implements binding.JsHooks {
  name: string;
  params: BuildParams & { resolveAlias: Record<string, string> };
  extOpts: RunLoadersOptions;
  parallelLoader: ReturnType<typeof createParallelLoader> | undefined;

  constructor(params: BuildParams & { resolveAlias: Record<string, string> }) {
    this.name = 'postcss';
    this.params = params;
    this.extOpts = {
      alias: params.resolveAlias,
      root: params.root,
    };
  }

  transform = async (
    _content: string,
    filename: string,
  ): Promise<{ content: string; type: 'css' | 'js' } | void> => {
    if (!isTargetFile(filename)) {
      return;
    }

    this.parallelLoader ||= createParallelLoader(
      path.resolve(__dirname, './render.js'),
    );
    const result = await this.parallelLoader.run({
      filename,
      extOpts: this.extOpts,
    });

    let css: string = '';

    if (result.result) {
      const buf = result.result[0];
      if (Buffer.isBuffer(buf)) {
        css = buf.toString('utf-8');
      } else {
        css = buf ?? '';
      }
    }

    return {
      content: css,
      type: 'css',
    };
  };
}

function getFilename(filePath: string) {
  let filename = '';
  try {
    filename = decodeURIComponent(url.parse(filePath).pathname || '');
  } catch (e) {
    return '';
  }

  return filename;
}

function isTargetFile(filePath: string) {
  let filename = getFilename(filePath);

  if (
    filename?.endsWith('.less') ||
    filename?.endsWith('.scss') ||
    filename?.endsWith('.css')
  ) {
    return true;
  }

  return false;
}
