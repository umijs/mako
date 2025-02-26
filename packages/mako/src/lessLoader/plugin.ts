import path from 'path';
import EnhancedResolve, { type ResolveFunctionAsync } from 'enhanced-resolve';

const trailingSlash = /[/\\]$/;

const IS_SPECIAL_MODULE_IMPORT = /^~[^/]+$/;

// `[drive_letter]:\` + `\\[server]\[share_name]\`
const IS_NATIVE_WIN32_PATH = /^[a-z]:[/\\]|^\\\\/i;

// Examples:
// - ~package
// - ~package/
// - ~@org
// - ~@org/
// - ~@org/package
// - ~@org/package/
const IS_MODULE_IMPORT =
  /^~([^/]+|[^/]+\/|@[^/]+[/][^/]+|@[^/]+\/?|@[^/]+[/][^/]+\/)$/;
const MODULE_REQUEST_REGEX = /^[^?]*~/;

export function createLessPlugin(less: LessStatic): Less.Plugin {
  const resolve = EnhancedResolve.create({
    conditionNames: ['less', 'style', '...'],
    mainFields: ['less', 'style', 'main', '...'],
    mainFiles: ['index', '...'],
    extensions: ['.less', '.css'],
    preferRelative: true,
  });

  class FileManager extends less.FileManager {
    supports(filename: string) {
      if (filename[0] === '/' || IS_NATIVE_WIN32_PATH.test(filename)) {
        return true;
      }

      if (this.isPathAbsolute(filename)) {
        return false;
      }

      return true;
    }

    supportsSync() {
      return false;
    }

    async resolveFilename(filename: string, currentDirectory: string) {
      // Less is giving us trailing slashes, but the context should have no trailing slash
      const context = currentDirectory.replace(trailingSlash, '');

      let request = filename;

      // A `~` makes the url an module
      if (MODULE_REQUEST_REGEX.test(filename)) {
        request = request.replace(MODULE_REQUEST_REGEX, '');
      }

      if (IS_MODULE_IMPORT.test(filename)) {
        request = request[request.length - 1] === '/' ? request : `${request}/`;
      }

      return this.resolveRequests(context, [...new Set([request, filename])]);
    }

    async resolveRequests(
      context: string,
      possibleRequests: string[],
    ): Promise<string> {
      if (possibleRequests.length === 0) {
        return Promise.reject();
      }

      let result;

      try {
        result = await asyncResolve(context, possibleRequests[0], resolve);
      } catch (error) {
        const [, ...tailPossibleRequests] = possibleRequests;

        if (tailPossibleRequests.length === 0) {
          throw error;
        }

        result = await this.resolveRequests(context, tailPossibleRequests);
      }

      return result;
    }

    async loadFile(
      filename: string,
      currentDirectory: string,
      options: Less.LoadFileOptions,
      environment: Less.Environment,
    ) {
      let result;

      try {
        if (IS_SPECIAL_MODULE_IMPORT.test(filename)) {
          const error = new Error() as any;
          error.type = 'Next';
          throw error;
        }

        result = await super.loadFile(
          filename,
          currentDirectory,
          options,
          environment,
        );
      } catch (error: any) {
        if (error.type !== 'File' && error.type !== 'Next') {
          return Promise.reject(error);
        }

        try {
          result = await this.resolveFilename(filename, currentDirectory);
        } catch (_error) {
          return Promise.reject(error);
        }

        // FIXME: need to add dependency
        // addDependency(result);

        return super.loadFile(result, currentDirectory, options, environment);
      }

      // @ts-ignore
      const absoluteFilename = path.isAbsolute(result.filename)
        ? result.filename
        : path.resolve('.', result.filename);

      // FIXME: need to add dependency
      // addDependency(path.normalize(absoluteFilename));

      return result;
    }
  }

  return {
    install(_lessInstance, pluginManager) {
      pluginManager.addFileManager(new FileManager());
    },
    minVersion: [3, 0, 0],
  };
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
