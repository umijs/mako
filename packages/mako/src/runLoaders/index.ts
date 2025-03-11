import fs from 'fs';
import * as EnhancedResolve from 'enhanced-resolve';
import * as loaderRunner from 'loader-runner';
import * as loaderUtils from 'loader-utils';

function createLoaderContext(options: {
  root: string;
  alias?: Record<string, string>;
}) {
  const getResolveContext = (loaderContext: any) => ({
    fileDependencies: {
      add: (d: string) => loaderContext.addDependency(d),
    },
    contextDependencies: {
      add: (d: string) => loaderContext.addContextDependency(d),
    },
    missingDependencies: {
      add: (d: string) => loaderContext.addMissingDependency(d),
    },
  });

  const defaultResolverOptions = {
    fileSystem: new EnhancedResolve.CachedInputFileSystem(fs, 60000),
    alias: options.alias,
  };

  const defaultResolver = EnhancedResolve.ResolverFactory.createResolver(
    defaultResolverOptions,
  );

  const ctx = {
    version: 2,
    rootContext: options.root,
    fs: new EnhancedResolve.CachedInputFileSystem(fs, 60000),
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
      defaultResolver.resolve(
        {},
        context,
        request,
        getResolveContext(this),
        callback,
      );
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
          resolver.resolve(
            {},
            context,
            request,
            getResolveContext(this),
            callback,
          );
        } else {
          return new Promise((resolve, reject) => {
            resolver.resolve(
              {},
              context,
              request,
              getResolveContext(this),
              (err, result) => {
                if (err) reject(err);
                else resolve(result);
              },
            );
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
