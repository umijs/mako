// mako/runtime/hmr plugin
!(function () {
  let currentParents = [];
  let currentChildModule;
  requireModule.hmrC = {};
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
      }
      return require(request);
    };
    Object.assign(fn, require);
    return fn;
  };
  const applyHotUpdate = (_chunkId, update) => {
    const { modules, removedModules } = update;
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
        if (module.hot._main) {
          location.reload();
        }
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
    const outdatedSelfAcceptedModules = [];
    for (const moduleId of outdatedModules) {
      const module = modulesRegistry[moduleId];
      if (module.hot._selfAccepted) {
        outdatedSelfAcceptedModules.push(module);
      }
    }
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
    registerModules(modules);
    for (const module of outdatedSelfAcceptedModules) {
      module.hot._requireSelf();
    }
  };
  const createModuleHotObject = (moduleId, me) => {
    const _main = currentChildModule !== moduleId;
    const hot = {
      _acceptedDependencies: {},
      _declinedDependencies: {},
      _selfAccepted: false,
      _selfDeclined: false,
      _selfInvalidated: false,
      _disposeHandlers: [],
      _requireSelf: function () {
        currentParents = me.parents.slice();
        currentChildModule = _main ? undefined : moduleId;
        requireModule(moduleId);
      },
      _main,
      active: true,
      accept() {
        this._selfAccepted = true;
      },
      dispose(callback) {
        this._disposeHandlers.push(callback);
      },
      invalidate() {},
      check() {
        const current_hash = requireModule.currentHash();
        return fetch(
          `${requireModule.publicPath}${current_hash}.hot-update.json`,
        )
          .then((res) => {
            return res.json();
          })
          .then((update) => {
            return Promise.all(
              update.c.map((chunk) => {
                let parts = chunk.split('.');
                let l = parts.length;
                let left = parts.slice(0, parts.length - 1).join('.');
                let ext = parts[l - 1];
                const hotChunkName = [
                  left,
                  current_hash,
                  'hot-update',
                  ext,
                ].join('.');
                return new Promise((done) => {
                  const url = `${requireModule.publicPath}${hotChunkName}`;
                  requireModule.loadScript(url, done);
                });
              }),
            );
          });
      },
      apply(update) {
        return applyHotUpdate(update);
      },
    };
    currentChildModule = undefined;
    return hot;
  };
  requireModule.hmrC.jsonp = (chunkId, update, promises) => {
    promises.push(
      new Promise((resolve) => {
        applyHotUpdate(chunkId, update);
        resolve();
      }),
    );
  };
  requireModule.hmrC.css = (chunkId, _update, promises) => {
    if (cssChunksIdToUrlMap[chunkId]) {
      promises.push(
        new Promise((resolve, reject) => {
          let url = cssChunksIdToUrlMap[chunkId];
          const fullUrl = requireModule.publicPath + url;
          const oldLink = requireModule.findStylesheet(url);
          if (oldLink) {
            const newLink = requireModule.createStylesheet(
              chunkId,
              `${fullUrl}?${Date.now()}`,
              oldLink,
              () => {
                newLink.rel = 'stylesheet';
                newLink.as = null;
                oldLink.parentNode.removeChild(oldLink);
                resolve();
              },
              reject,
            );
            newLink.rel = 'prereload';
            newLink.as = 'style';
          }
        }),
      );
    }
  };
  requireModule.requireInterceptors.push((options) => {
    const orginRequire = options.require;
    options.module.hot = createModuleHotObject(options.id, options.module);
    options.module.meta = {
      hot: options.module.hot,
    };
    options.module.parents = currentParents;
    currentParents = [];
    options.module.children = [];
    options.require = createHmrRequire(options.require, options.id);
    options.require.currentHash = () => {
      return orginRequire._h;
    };
  });
  requireModule.applyHotUpdate = (chunkId, update, runtime) => {
    runtime(requireModule);
    return Promise.all(
      Object.keys(requireModule.hmrC).reduce(function (promises, key) {
        requireModule.hmrC[key](chunkId, update, promises);
        return promises;
      }, []),
    );
  };
})();
