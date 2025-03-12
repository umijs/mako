import fs from 'fs';
import * as EnhancedResolve from 'enhanced-resolve';
import * as loaderRunner from 'loader-runner';
import * as loaderUtils from 'loader-utils';

const fileSystem = new EnhancedResolve.CachedInputFileSystem(fs, 60 * 1000);

function createLoaderContext(options: {
  root: string;
  alias?: Record<string, string>;
}) {
  const defaultResolverOptions = {
    fileSystem,
    alias: options.alias,
  };

  const defaultResolver = EnhancedResolve.ResolverFactory.createResolver(
    defaultResolverOptions,
  );

  const ctx = {
    version: 2,
    rootContext: options.root,
    fs: fileSystem,
    getOptions(): Record<string, any> {
      const query = (this as unknown as loaderRunner.ExtendedLoaderContext)
        .query;
      if (typeof query === 'string' && query !== '') {
        return loaderUtils.parseQuery(query);
      }

      if (!query || typeof query !== 'object') {
        return {};
      }

      return query;
    },
    resolve(context: string, request: string, callback: any) {
      defaultResolver.resolve({}, context, request, {}, callback);
    },
    getResolve(options: EnhancedResolve.ResolveOptions) {
      const resolver = options
        ? EnhancedResolve.ResolverFactory.createResolver({
            ...defaultResolverOptions,
            ...options,
          })
        : defaultResolver;

      return (context: string, request: string, callback: any) => {
        if (callback) {
          resolver.resolve({}, context, request, {}, callback);
        } else {
          return new Promise((resolve, reject) => {
            resolver.resolve({}, context, request, {}, (err, result) => {
              if (err) reject(err);
              else resolve(result);
            });
          });
        }
      };
    },
    target: 'web',
    getLogger() {
      return console;
    },
  };

  return ctx;
}

export interface RunLoadersOptions {
  root: string;
  alias?: Record<string, string>;
}

export function runLoaders(
  options: {
    resource: string;
    loaders: any[];
  } & RunLoadersOptions,
): Promise<loaderRunner.RunLoaderResult> {
  return new Promise((resolve, reject) => {
    loaderRunner.runLoaders(
      {
        resource: options.resource,
        readResource: fs.readFile.bind(fs),
        context: createLoaderContext({
          root: options.root,
          alias: options.alias,
        }),
        loaders: options.loaders,
      },
      (error, data) => {
        if (error) {
          reject(error);
          return;
        }
        resolve(data);
      },
    );
  });
}
