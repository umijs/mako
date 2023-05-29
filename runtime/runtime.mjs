export function createRuntime(makoModules, entryModuleId) {
  const modulesRegistry = {};

  function requireModule(moduleId) {
    if (modulesRegistry[moduleId] !== undefined) {
      return modulesRegistry[moduleId].exports;
    }

    const module = {
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
      hmrHandler(execOptions);
      execOptions.factory(
        execOptions.module,
        execOptions.module.exports,
        execOptions.require,
      );
    } catch (e) {
      console.error(`Error require module '${moduleId}':`, e);
      delete modulesRegistry[moduleId];
    }

    return module.exports;
  }

  // hmr
  let currentParents = [];
  let currentChildModule;
  const hmrHandler = (options) => {
    options.module.hot = createModuleHotObject(options.id, options.module);
    options.module.parents = currentParents;
    currentParents = [];
    options.module.children = [];
    options.require = createHmrRequire(options.require, options.id);
  };
  const createHmrRequire = (require, moduleId) => {
    const me = modulesRegistry[moduleId];
    if (!me) return require;
    const fn = (request) => {
      if (me.hot.active) {
        if (modulesRegistry[request]) {
          const parents = modulesRegistry[request].parents;
          if (!parents.includes(moduleId)) {
            parents.push(moduleId);
          }
        } else {
          currentParents = [moduleId];
          currentChildModule = request;
        }
        if (!me.children.includes(request)) {
          me.children.push(request);
        }
      } else {
        // TODO
      }
      return require(request);
    };
    // TODO: fn.ensure 需要确保依赖关系
    fn.ensure = ensure;
    return fn;
  };
  const createModuleHotObject = (moduleId, me) => {
    const hot = {
      _acceptedDependencies: {},
      _declinedDependencies: {},
      _selfAccepted: false,
      _selfDeclined: false,
      _selfInvalidated: false,
      _disposeHandlers: [],
      _requireSelf: function () {
        currentParents = me.parents.slice();
        requireModule(moduleId);
      },
      active: true,
      accept() {
        this._selfAccepted = true;
      },
      dispose(callback) {
        this._disposeHandlers.push(callback);
      },
      invalidate() {},
      check() {
        fetch('/hot-update.json')
          .then((res) => {
            return res.json();
          })
          .then((update) => {
            if (update) {
              hot.apply(update);
            }
          });
      },
      apply(update) {
        const { modules, removedModules } = update;

        // get outdated modules
        const outdatedModules = [];
        for (const moduleId of Object.keys(modules)) {
          if (!modulesRegistry[moduleId]) continue;
          if (outdatedModules.includes(moduleId)) continue;
          outdatedModules.push(moduleId);
          const queue = [moduleId];
          while (queue.length) {
            const item = queue.pop();
            const module = modulesRegistry[item];
            if (!module) continue;
            if (module.hot._selfAccepted) {
              continue;
            }
            for (const parentModule of module.parents) {
              if (outdatedModules.includes(parentModule)) continue;
              outdatedModules.push(parentModule);
              queue.push(parentModule);
            }
          }
        }

        // get self accepted modules
        const outdatedSelfAcceptedModules = [];
        for (const moduleId of outdatedModules) {
          const module = modulesRegistry[moduleId];
          if (module.hot._selfAccepted) {
            outdatedSelfAcceptedModules.push(module);
          }
        }

        // dispose
        for (const moduleId of outdatedModules) {
          const module = modulesRegistry[moduleId];
          for (const handler of module.hot._disposeHandlers) {
            handler();
          }
          module.hot.active = false;
          delete modulesRegistry[moduleId];
          for (const childModule of module.children) {
            const child = modulesRegistry[childModule];
            if (!child) continue;
            const idx = child.parents.indexOf(moduleId);
            if (idx !== -1) {
              child.parents.splice(idx, 1);
            }
          }
        }

        // apply
        registerModules(modules);
        for (const module of outdatedSelfAcceptedModules) {
          module.hot._requireSelf();
        }
      },
    };
    return hot;
  };

  // chunk and async load
  const installedChunks = {};
  const ensure = (chunkId) => {
    let data = installedChunks[chunkId];
    if (data === 0) return Promise.resolve();
    if (data) {
      // [resolve, reject, promise]
      return data[2];
    } else {
      const promise = new Promise((resolve, reject) => {
        data = installedChunks[chunkId] = [resolve, reject];
      });
      data[2] = promise;
      // TODO: support public path
      const url = `/${chunkId}.async.js`;
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
      load(url, onLoadEnd, `chunk-${chunkId}`);
      return promise;
    }
  };

  const inProgress = {};
  const load = (url, done, key) => {
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
    // 可能不需要，有 timeout 属性了
    const timeout = setTimeout(
      onLoadEnd.bind(null, undefined, { type: 'timeout', target: script }),
      120000,
    );
    script.onerror = onLoadEnd.bind(null, script.onerror);
    script.onload = onLoadEnd.bind(null, script.onload);
    document.head.appendChild(script);
  };

  const jsonpCallback = (data) => {
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

  requireModule.ensure = ensure;
  requireModule(entryModuleId);

  return {
    requireModule,
    _modulesRegistry: modulesRegistry,
    _jsonpCallback: jsonpCallback,
  };
}
