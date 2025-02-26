import fs from 'fs';
import path from 'path';
import url from 'url';
import EnhancedResolve, { type ResolveFunctionAsync } from 'enhanced-resolve';
import type { Importer, ImporterResult } from 'sass';

export function createImporter(
  filename: string,
  implementation: any,
): Importer {
  return {
    async canonicalize(originalUrl, context) {
      const { fromImport } = context;
      const prev = context.containingUrl
        ? url.fileURLToPath(context.containingUrl.toString())
        : filename;

      const resolver = getResolver(implementation?.compileStringAsync);
      try {
        const result = await resolver(prev, originalUrl, fromImport);
        return url.pathToFileURL(result) as URL;
      } catch (err) {
        return null;
      }
    },
    async load(canonicalUrl) {
      const ext = path.extname(canonicalUrl.pathname);

      let syntax;

      if (ext && ext.toLowerCase() === '.scss') {
        syntax = 'scss';
      } else if (ext && ext.toLowerCase() === '.sass') {
        syntax = 'indented';
      } else if (ext && ext.toLowerCase() === '.css') {
        syntax = 'css';
      } else {
        // Fallback to default value
        syntax = 'scss';
      }

      try {
        const contents = await new Promise((resolve, reject) => {
          const canonicalPath = url.fileURLToPath(canonicalUrl);

          fs.readFile(
            canonicalPath,
            {
              encoding: 'utf8',
            },
            (err, content) => {
              if (err) {
                reject(err);
                return;
              }

              resolve(content);
            },
          );
        });

        return {
          contents,
          syntax,
          sourceMapUrl: canonicalUrl,
        } as ImporterResult;
      } catch (err) {
        return null;
      }
    },
  };
}

function getResolver(compileStringAsync: any) {
  const isModernSass = typeof compileStringAsync !== 'undefined';

  const importResolve = EnhancedResolve.create({
    conditionNames: ['sass', 'style', '...'],
    mainFields: ['sass', 'style', 'main', '...'],
    mainFiles: ['_index.import', '_index', 'index.import', 'index', '...'],
    extensions: ['.sass', '.scss', '.css'],
    restrictions: [/\.((sa|sc|c)ss)$/i],
    preferRelative: true,
  });
  const moduleResolve = EnhancedResolve.create({
    conditionNames: ['sass', 'style', '...'],
    mainFields: ['sass', 'style', 'main', '...'],
    mainFiles: ['_index', 'index', '...'],
    extensions: ['.sass', '.scss', '.css'],
    restrictions: [/\.((sa|sc|c)ss)$/i],
    preferRelative: true,
  });

  return (context: string, request: string, fromImport: boolean) => {
    if (!isModernSass && !path.isAbsolute(context)) {
      return Promise.reject();
    }

    const originalRequest = request;
    const isFileScheme = originalRequest.slice(0, 5).toLowerCase() === 'file:';

    if (isFileScheme) {
      try {
        request = url.fileURLToPath(originalRequest);
      } catch (error) {
        request = request.slice(7);
      }
    }

    let resolutionMap: any[] = [];

    const webpackPossibleRequests = getPossibleRequests(request, fromImport);

    resolutionMap = resolutionMap.concat({
      resolve: fromImport ? importResolve : moduleResolve,
      context: path.dirname(context),
      possibleRequests: webpackPossibleRequests,
    });

    return startResolving(resolutionMap);
  };
}

const MODULE_REQUEST_REGEX = /^[^?]*~/;

// Examples:
// - ~package
// - ~package/
// - ~@org
// - ~@org/
// - ~@org/package
// - ~@org/package/
const IS_MODULE_IMPORT =
  /^~([^/]+|[^/]+\/|@[^/]+[/][^/]+|@[^/]+\/?|@[^/]+[/][^/]+\/)$/;

const IS_PKG_SCHEME = /^pkg:/i;

function getPossibleRequests(url: string, fromImport: boolean) {
  console.log('getPossibleRequests', url);
  let request = url;

  if (MODULE_REQUEST_REGEX.test(url)) {
    request = request.replace(MODULE_REQUEST_REGEX, '');
  }

  if (IS_PKG_SCHEME.test(url)) {
    request = `${request.slice(4)}`;

    return [...new Set([request, url])];
  }

  if (IS_MODULE_IMPORT.test(url) || IS_PKG_SCHEME.test(url)) {
    request = request[request.length - 1] === '/' ? request : `${request}/`;

    return [...new Set([request, url])];
  }

  const extension = path.extname(request).toLowerCase();

  if (extension === '.css') {
    return fromImport ? [] : [url];
  }

  const dirname = path.dirname(request);
  const normalizedDirname = dirname === '.' ? '' : `${dirname}/`;
  const basename = path.basename(request);
  const basenameWithoutExtension = path.basename(request, extension);

  return [
    ...new Set(
      ([] as any[])
        .concat(
          fromImport
            ? [
                `${normalizedDirname}_${basenameWithoutExtension}.import${extension}`,
                `${normalizedDirname}${basenameWithoutExtension}.import${extension}`,
              ]
            : [],
        )
        .concat([
          `${normalizedDirname}_${basename}`,
          `${normalizedDirname}${basename}`,
        ])
        .concat([url]),
    ),
  ];
}

async function startResolving(resolutionMap: any[]) {
  if (resolutionMap.length === 0) {
    return Promise.reject();
  }

  const [{ possibleRequests }] = resolutionMap;

  if (possibleRequests.length === 0) {
    return Promise.reject();
  }

  const [{ resolve, context }] = resolutionMap;

  try {
    return await asyncResolve(context, possibleRequests[0], resolve);
  } catch (_ignoreError) {
    const [, ...tailResult] = possibleRequests;

    if (tailResult.length === 0) {
      const [, ...tailResolutionMap] = resolutionMap;

      return startResolving(tailResolutionMap);
    }

    resolutionMap[0].possibleRequests = tailResult;

    return startResolving(resolutionMap);
  }
}

function asyncResolve(
  context: string,
  path: string,
  resolve: ResolveFunctionAsync,
): Promise<string> {
  return new Promise((res, rej) => {
    resolve(context, path, (err, result) => {
      if (err) {
        rej(err);
        return;
      }

      res(result as string);
    });
  });
}
