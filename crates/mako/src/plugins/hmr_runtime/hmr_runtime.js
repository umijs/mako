'use strict';
// mako/runtime/hmr plugin
!(function () {
  requireModule._h = '_%full_hash%_';
  requireModule.currentHash = function () {
    return requireModule._h;
  };
})();
!(function () {
  var currentParents = [];
  var currentChildModule;
  requireModule.hmrC = {};
  var createHmrRequire = function (require, moduleId) {
    var me = modulesRegistry[moduleId];
    if (!me) return require;
    var fn = function (request) {
      if (me.hot.active) {
        if (modulesRegistry[request]) {
          var parents = modulesRegistry[request].parents;
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
    var createPropertyDescriptor = function (name) {
      return {
        configurable: true,
        enumerable: true,
        get: function () {
          return require[name];
        },
        set: function (value) {
          require[name] = value;
        },
      };
    };
    for (var name in require) {
      if (Object.prototype.hasOwnProperty.call(require, name)) {
        Object.defineProperty(fn, name, createPropertyDescriptor(name));
      }
    }
    return fn;
  };
  var applyHotUpdate = function (_chunkId, update) {
    var modules = update.modules,
      removedModules = update.removedModules;
    var outdatedModules = [];
    for (var _i = 0, _a = Object.keys(modules); _i < _a.length; _i++) {
      var moduleId = _a[_i];
      if (!modulesRegistry[moduleId]) continue;
      if (outdatedModules.includes(moduleId)) continue;
      outdatedModules.push(moduleId);
      var queue = [moduleId];
      while (queue.length) {
        var item = queue.pop();
        var module = modulesRegistry[item];
        if (!module) continue;
        if (module.hot._main) {
          location.reload();
        }
        if (module.hot._selfAccepted) {
          continue;
        }
        for (var _b = 0, _c = module.parents; _b < _c.length; _b++) {
          var parentModule = _c[_b];
          if (outdatedModules.includes(parentModule)) continue;
          outdatedModules.push(parentModule);
          queue.push(parentModule);
        }
      }
    }
    var outdatedSelfAcceptedModules = [];
    for (
      var _d = 0, outdatedModules_1 = outdatedModules;
      _d < outdatedModules_1.length;
      _d++
    ) {
      var moduleId = outdatedModules_1[_d];
      var module = modulesRegistry[moduleId];
      if (module.hot._selfAccepted) {
        outdatedSelfAcceptedModules.push(module);
      }
    }
    for (
      var _e = 0, outdatedModules_2 = outdatedModules;
      _e < outdatedModules_2.length;
      _e++
    ) {
      var moduleId = outdatedModules_2[_e];
      var module = modulesRegistry[moduleId];
      for (var _f = 0, _g = module.hot._disposeHandlers; _f < _g.length; _f++) {
        var handler = _g[_f];
        handler();
      }
      module.hot.active = false;
      delete modulesRegistry[moduleId];
      for (var _j = 0, _k = module.children; _j < _k.length; _j++) {
        var childModule = _k[_j];
        var child = modulesRegistry[childModule];
        if (!child) continue;
        var idx = child.parents.indexOf(moduleId);
        if (idx !== -1) {
          child.parents.splice(idx, 1);
        }
      }
    }
    registerModules(modules);
    for (
      var _l = 0, outdatedSelfAcceptedModules_1 = outdatedSelfAcceptedModules;
      _l < outdatedSelfAcceptedModules_1.length;
      _l++
    ) {
      var module = outdatedSelfAcceptedModules_1[_l];
      module.hot._requireSelf();
    }
  };
  var createModuleHotObject = function (moduleId, me) {
    var _main = currentChildModule !== moduleId;
    var hot = {
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
      _main: _main,
      active: true,
      accept: function () {
        this._selfAccepted = true;
      },
      dispose: function (callback) {
        this._disposeHandlers.push(callback);
      },
      invalidate: function () {},
      updateChunksUrlMap: function () {
        var current_hash = requireModule.currentHash();
        return fetch(
          ''
            .concat(requireModule.publicPath)
            .concat(current_hash, '.hot-update-url-map.json'),
        )
          .then(function (res) {
            return res.json();
          })
          .then(function (chunksUrlMap) {
            Object.assign(chunksIdToUrlMap, chunksUrlMap.js);
            Object.assign(cssChunksIdToUrlMap, chunksUrlMap.css);
          });
      },
      check: function () {
        var current_hash = requireModule.currentHash();
        return fetch(
          ''
            .concat(requireModule.publicPath)
            .concat(current_hash, '.hot-update.json'),
        )
          .then(function (res) {
            return res.json();
          })
          .then(function (update) {
            return Promise.all(
              update.c.map(function (chunk) {
                var parts = chunk.split('.');
                var l = parts.length;
                var left = parts.slice(0, parts.length - 1).join('.');
                var ext = parts[l - 1];
                var hotChunkName = [left, current_hash, 'hot-update', ext].join(
                  '.',
                );
                return new Promise(function (done) {
                  var url = ''
                    .concat(requireModule.publicPath)
                    .concat(hotChunkName);
                  requireModule.loadScript(url, done);
                });
              }),
            );
          });
      },
      apply: function (update) {
        return applyHotUpdate(update);
      },
    };
    currentChildModule = undefined;
    return hot;
  };
  requireModule.hmrC.jsonp = function (chunkId, update, promises) {
    promises.push(
      new Promise(function (resolve) {
        applyHotUpdate(chunkId, update);
        resolve();
      }),
    );
  };
  requireModule.hmrC.css = function (chunkId, _update, promises) {
    if (cssChunksIdToUrlMap[chunkId]) {
      promises.push(
        new Promise(function (resolve, reject) {
          var url = cssChunksIdToUrlMap[chunkId];
          var fullUrl = requireModule.publicPath + url;
          var oldLink = requireModule.findStylesheet(url);
          if (oldLink) {
            var newLink_1 = requireModule.createStylesheet(
              chunkId,
              ''.concat(fullUrl, '?').concat(Date.now()),
              oldLink,
              function () {
                newLink_1.rel = 'stylesheet';
                newLink_1.as = null;
                oldLink.parentNode.removeChild(oldLink);
                resolve();
              },
              reject,
            );
            newLink_1.rel = 'prereload';
            newLink_1.as = 'style';
          }
        }),
      );
    }
  };
  requireModule.requireInterceptors.push(function (options) {
    var originRequire = options.require;
    options.module.hot = createModuleHotObject(options.id, options.module);
    options.module.meta = {
      hot: options.module.hot,
    };
    options.module.parents = currentParents;
    currentParents = [];
    options.module.children = [];
    options.require = createHmrRequire(options.require, options.id);
    options.require.currentHash = function () {
      return originRequire._h;
    };
  });
  requireModule.applyHotUpdate = function (chunkId, update, runtime) {
    runtime(requireModule);
    return Promise.all(
      Object.keys(requireModule.hmrC).reduce(function (promises, key) {
        requireModule.hmrC[key](chunkId, update, promises);
        return promises;
      }, []),
    );
  };
})();
