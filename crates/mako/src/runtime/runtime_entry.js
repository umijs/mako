function createRuntime(makoModules, entryModuleId) {
  const modulesRegistry = {};

  function requireModule(moduleId) {
    if (moduleId === '$$IGNORED$$') return {};
    const cachedModule = modulesRegistry[moduleId];

    if (cachedModule !== undefined) {
      if (cachedModule.error) {
        throw cachedModule.error;
      }

      return cachedModule.exports;
    }

    const module = {
      id: moduleId,
      exports: {},
    };
    modulesRegistry[moduleId] = module;

    try {
      const execOptions = {
        id: moduleId,
        module,
        factory: makoModules[moduleId],
        require: requireModule,
      };

      requireModule.requireInterceptors.forEach((interceptor) => {
        interceptor(execOptions);
      });
      execOptions.factory.call(
        execOptions.module.exports,
        execOptions.module,
        execOptions.module.exports,
        execOptions.require,
      );
    } catch (e) {
      modulesRegistry[moduleId].error = e;
      throw e;
    }

    return module.exports;
  }

  // module execution interceptor
  requireModule.requireInterceptors = [];

  /* mako/runtime/ensure chunk */
  !(function () {
    requireModule.chunkEnsures = {};
    // This file contains only the entry chunk.
    // The chunk loading function for additional chunks
    requireModule.ensure = function (chunkId) {
      return Promise.all(
        Object.keys(requireModule.chunkEnsures).reduce(function (
          promises,
          key,
        ) {
          requireModule.chunkEnsures[key](chunkId, promises);
          return promises;
        }, []),
      );
    };
  })();

  /* mako/runtime/ensure load js Chunk */
  !(function () {
    const installedChunks = (requireModule.jsonpInstalled = {});

    requireModule.chunkEnsures.jsonp = (chunkId, promises) => {
      let data = installedChunks[chunkId];
      if (data === 0) return;

      if (data) {
        //     0       1        2
        // [resolve, reject, promise]
        promises.push(data[2]);
      } else {
        const promise = new Promise((resolve, reject) => {
          data = installedChunks[chunkId] = [resolve, reject];
        });
        promises.push((data[2] = promise));
        const url = requireModule.publicPath + chunksIdToUrlMap[chunkId];
        const error = new Error();
        const onLoadEnd = (event) => {
          data = installedChunks[chunkId];
          if (data !== 0) installedChunks[chunkId] = undefined;
          if (data) {
            const errorType = event?.type;
            const src = event?.target?.src;
            error.message = `Loading chunk ${chunkId} failed. (${errorType} : ${src})`;
            error.name = 'ChunkLoadError';
            error.type = errorType;
            data[1](error);
          }
        };
        // load
        requireModule.loadScript(url, onLoadEnd, `chunk-${chunkId}`);
        return promise;
      }
    };
  })();
  // chunk and async load

  /* mako/runtime/load script */
  !(function () {
    const inProgress = {};
    requireModule.loadScript = (url, done, key) => {
      if (inProgress[url]) {
        return inProgress[url].push(done);
      }
      const script = document.createElement('script');
      script.timeout = 120;
      script.src = url;
      inProgress[url] = [done];
      const onLoadEnd = (prev, event) => {
        clearTimeout(timeout);
        const doneFns = inProgress[url];
        delete inProgress[url];
        script.parentNode?.removeChild(script);
        if (doneFns) {
          doneFns.forEach(function (fn) {
            return fn(event);
          });
        }
        if (prev) return prev(event);
      };
      // May not be needed, already has timeout attributes
      const timeout = setTimeout(
        onLoadEnd.bind(null, undefined, { type: 'timeout', target: script }),
        120000,
      );
      script.onerror = onLoadEnd.bind(null, script.onerror);
      script.onload = onLoadEnd.bind(null, script.onload);
      document.head.appendChild(script);
    };
  })();
  /* mako/runtime/ensure load css chunk */
  const cssChunksIdToUrlMap = {};
  !(function () {
    const installedChunks = (requireModule.cssInstalled = {});
    // __CSS_CHUNKS_URL_MAP
    requireModule.findStylesheet = function (url) {
      return Array.from(
        document.querySelectorAll('link[href][rel=stylesheet]'),
      ).find((link) => {
        // why not use link.href?
        // because link.href contains hostname
        const [linkUrl] = link.getAttribute('href').split('?');

        return linkUrl === url || linkUrl === requireModule.publicPath + url;
      });
    };

    requireModule.createStylesheet = function (
      chunkId,
      url,
      oldTag,
      resolve,
      reject,
    ) {
      const link = document.createElement('link');

      link.rel = 'stylesheet';
      link.type = 'text/css';
      link.href = url;
      link.onerror = link.onload = function (event) {
        // avoid mem leaks, from webpack
        link.onerror = link.onload = null;

        if (event.type === 'load') {
          // finished loading css chunk
          installedChunks[chunkId] = 0;
          resolve();
        } else {
          // throw error and reset state
          delete installedChunks[chunkId];
          const errorType = event?.type;
          const realHref = event?.target?.href;
          const err = new Error(
            'Loading CSS chunk ' + chunkId + ' failed.\n(' + realHref + ')',
          );

          err.code = 'CSS_CHUNK_LOAD_FAILED';
          err.type = errorType;
          err.request = realHref;
          link.parentNode.removeChild(link);
          reject(err);
        }
      };

      if (oldTag) {
        oldTag.parentNode.insertBefore(link, oldTag.nextSibling);
      } else {
        document.head.appendChild(link);
      }

      return link;
    };

    requireModule.chunkEnsures.css = (chunkId, promises) => {
      if (installedChunks[chunkId]) {
        // still pending, avoid duplicate promises
        promises.push(installedChunks[chunkId]);
      } else if (
        installedChunks[chunkId] !== 0 &&
        cssChunksIdToUrlMap[chunkId]
      ) {
        // load chunk and save promise
        installedChunks[chunkId] = new Promise((resolve, reject) => {
          const url = cssChunksIdToUrlMap[chunkId];
          const fullUrl = requireModule.publicPath + url;

          if (requireModule.findStylesheet(url)) {
            // already loaded
            resolve();
          } else {
            // load new css chunk
            requireModule.createStylesheet(
              chunkId,
              fullUrl,
              null,
              resolve,
              reject,
            );
          }
        });
        promises.push(installedChunks[chunkId]);
        return promises;
      }
    };
  })();

  const jsonpCallback = (data) => {
    const installedChunks = requireModule.jsonpInstalled;

    const chunkIds = data[0];
    const modules = data[1];
    if (chunkIds.some((id) => installedChunks[id] !== 0)) {
      registerModules(modules);
    }
    for (const id of chunkIds) {
      if (installedChunks[id]) {
        installedChunks[id][0]();
      }
      installedChunks[id] = 0;
    }
  };

  const registerModules = (modules) => {
    for (const id in modules) {
      makoModules[id] = modules[id];
    }
  };

  requireModule._h = '_%full_hash%_';
  requireModule.currentHash = () => {
    return requireModule._h;
  };

  // __inject_runtime_code__

  // __WASM_REQUIRE_SUPPORT
  // __REQUIRE_ASYNC_MODULE_SUPPORT
  // __BEFORE_ENTRY

  requireModule(entryModuleId);

  // __AFTER_ENTRY

  return {
    requireModule,
    _modulesRegistry: modulesRegistry,
    _jsonpCallback: jsonpCallback,
    _makoModuleHotUpdate: requireModule.applyHotUpdate,
  };
}

const runtime = createRuntime({}, '_%main%_');
globalThis.jsonpCallback = runtime._jsonpCallback;
globalThis.modulesRegistry = runtime._modulesRegistry;
globalThis.makoModuleHotUpdate = runtime._makoModuleHotUpdate;
