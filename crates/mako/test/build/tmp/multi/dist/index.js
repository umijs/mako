var chunksIdToUrlMap = {
    "chunk-1.ts": "chunk-1_ts-async.js"
};
var cssChunksIdToUrlMap = {};
var e = "index.ts";
var cssInstalledChunks = {
    "index.ts": 0
};
var m = {
    "index.ts": function(module, exports, require) {
        (async ()=>{
            await Promise.all([
                require.ensure("chunk-1.ts")
            ]).then(require.bind(require, "chunk-1.ts"));
        })();
    }
};
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
            exports: {}
        };
        modulesRegistry[moduleId] = module;
        try {
            const execOptions = {
                id: moduleId,
                module,
                factory: makoModules[moduleId],
                require: requireModule
            };
            requireModule.requireInterceptors.forEach((interceptor)=>{
                interceptor(execOptions);
            });
            execOptions.factory.call(execOptions.module.exports, execOptions.module, execOptions.module.exports, execOptions.require);
        } catch (e) {
            modulesRegistry[moduleId].error = e;
            throw e;
        }
        return module.exports;
    }
    requireModule.requireInterceptors = [];
    !(function() {
        requireModule.chunkEnsures = {};
        requireModule.ensure = function(chunkId) {
            return Promise.all(Object.keys(requireModule.chunkEnsures).reduce(function(promises, key) {
                requireModule.chunkEnsures[key](chunkId, promises);
                return promises;
            }, []));
        };
    })();
    !(function() {
        const installedChunks = (requireModule.jsonpInstalled = {});
        requireModule.chunkEnsures.jsonp = (chunkId, promises)=>{
            let data = installedChunks[chunkId];
            if (data === 0) return;
            if (data) {
                promises.push(data[2]);
            } else {
                const promise = new Promise((resolve, reject)=>{
                    data = installedChunks[chunkId] = [
                        resolve,
                        reject
                    ];
                });
                promises.push((data[2] = promise));
                const url = requireModule.publicPath + chunksIdToUrlMap[chunkId];
                const error = new Error();
                const onLoadEnd = (event)=>{
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
                requireModule.loadScript(url, onLoadEnd, `chunk-${chunkId}`);
                return promise;
            }
        };
    })();
    !(function() {
        const inProgress = {};
        requireModule.loadScript = (url, done, key)=>{
            if (inProgress[url]) {
                return inProgress[url].push(done);
            }
            const script = document.createElement('script');
            script.timeout = 120;
            script.src = url;
            inProgress[url] = [
                done
            ];
            const onLoadEnd = (prev, event)=>{
                clearTimeout(timeout);
                const doneFns = inProgress[url];
                delete inProgress[url];
                script.parentNode?.removeChild(script);
                if (doneFns) {
                    doneFns.forEach(function(fn) {
                        return fn(event);
                    });
                }
                if (prev) return prev(event);
            };
            const timeout = setTimeout(onLoadEnd.bind(null, undefined, {
                type: 'timeout',
                target: script
            }), 120000);
            script.onerror = onLoadEnd.bind(null, script.onerror);
            script.onload = onLoadEnd.bind(null, script.onload);
            document.head.appendChild(script);
        };
    })();
    !(function() {
        requireModule.cssInstalled = cssInstalledChunks;
        requireModule.findStylesheet = function(url) {
            return Array.from(document.querySelectorAll('link[href][rel=stylesheet]')).find((link)=>{
                const [linkUrl] = link.getAttribute('href').split('?');
                return linkUrl === url || linkUrl === requireModule.publicPath + url;
            });
        };
        requireModule.createStylesheet = function(chunkId, url, oldTag, resolve, reject) {
            const link = document.createElement('link');
            link.rel = 'stylesheet';
            link.type = 'text/css';
            link.href = url;
            link.onerror = link.onload = function(event) {
                link.onerror = link.onload = null;
                if (event.type === 'load') {
                    cssInstalledChunks[chunkId] = 0;
                    resolve();
                } else {
                    delete cssInstalledChunks[chunkId];
                    const errorType = event?.type;
                    const realHref = event?.target?.href;
                    const err = new Error('Loading CSS chunk ' + chunkId + ' failed.\n(' + realHref + ')');
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
        requireModule.chunkEnsures.css = (chunkId, promises)=>{
            if (cssInstalledChunks[chunkId]) {
                promises.push(cssInstalledChunks[chunkId]);
            } else if (cssInstalledChunks[chunkId] !== 0 && cssChunksIdToUrlMap[chunkId]) {
                cssInstalledChunks[chunkId] = new Promise((resolve, reject)=>{
                    const url = cssChunksIdToUrlMap[chunkId];
                    const fullUrl = requireModule.publicPath + url;
                    if (requireModule.findStylesheet(url)) {
                        resolve();
                    } else {
                        requireModule.createStylesheet(chunkId, fullUrl, null, resolve, reject);
                    }
                });
                promises.push(cssInstalledChunks[chunkId]);
                return promises;
            }
        };
    })();
    const jsonpCallback = (data)=>{
        const installedChunks = requireModule.jsonpInstalled;
        const chunkIds = data[0];
        const modules = data[1];
        if (chunkIds.some((id)=>installedChunks[id] !== 0)) {
            registerModules(modules);
        }
        for (const id of chunkIds){
            if (installedChunks[id]) {
                installedChunks[id][0]();
            }
            installedChunks[id] = 0;
        }
    };
    const registerModules = (modules)=>{
        for(const id in modules){
            makoModules[id] = modules[id];
        }
    };
    !function() {
        requireModule.publicPath = "/";
    }();
    !function() {
        registerModules({
            "@swc/helpers/_/_interop_require_default": function(module, exports, require) {
                Object.defineProperty(exports, "__esModule", {
                    value: true
                });
                function _export(target, all) {
                    for(var name in all)Object.defineProperty(target, name, {
                        enumerable: true,
                        get: all[name]
                    });
                }
                _export(exports, {
                    _interop_require_default: function() {
                        return _interop_require_default;
                    },
                    _: function() {
                        return _interop_require_default;
                    }
                });
                function _interop_require_default(obj) {
                    return obj && obj.__esModule ? obj : {
                        default: obj
                    };
                }
            },
            "@swc/helpers/_/_interop_require_wildcard": function(module, exports, require) {
                Object.defineProperty(exports, "__esModule", {
                    value: true
                });
                function _export(target, all) {
                    for(var name in all)Object.defineProperty(target, name, {
                        enumerable: true,
                        get: all[name]
                    });
                }
                _export(exports, {
                    _interop_require_wildcard: function() {
                        return _interop_require_wildcard;
                    },
                    _: function() {
                        return _interop_require_wildcard;
                    }
                });
                function _getRequireWildcardCache(nodeInterop) {
                    if (typeof WeakMap !== "function") return null;
                    var cacheBabelInterop = new WeakMap();
                    var cacheNodeInterop = new WeakMap();
                    return (_getRequireWildcardCache = function(nodeInterop) {
                        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
                    })(nodeInterop);
                }
                function _interop_require_wildcard(obj, nodeInterop) {
                    if (!nodeInterop && obj && obj.__esModule) return obj;
                    if (obj === null || typeof obj !== "object" && typeof obj !== "function") return {
                        default: obj
                    };
                    var cache = _getRequireWildcardCache(nodeInterop);
                    if (cache && cache.has(obj)) return cache.get(obj);
                    var newObj = {};
                    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
                    for(var key in obj){
                        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
                            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
                            if (desc && (desc.get || desc.set)) Object.defineProperty(newObj, key, desc);
                            else newObj[key] = obj[key];
                        }
                    }
                    newObj.default = obj;
                    if (cache) cache.set(obj, newObj);
                    return newObj;
                }
            },
            "@swc/helpers/_/_export_star": function(module, exports, require) {
                Object.defineProperty(exports, "__esModule", {
                    value: true
                });
                function _export(target, all) {
                    for(var name in all)Object.defineProperty(target, name, {
                        enumerable: true,
                        get: all[name]
                    });
                }
                _export(exports, {
                    _export_star: function() {
                        return _export_star;
                    },
                    _: function() {
                        return _export_star;
                    }
                });
                function _export_star(from, to) {
                    Object.keys(from).forEach(function(k) {
                        if (k !== "default" && !Object.prototype.hasOwnProperty.call(to, k)) {
                            Object.defineProperty(to, k, {
                                enumerable: true,
                                get: function() {
                                    return from[k];
                                }
                            });
                        }
                    });
                    return from;
                }
            }
        });
    }();
    const exports = requireModule(entryModuleId);
    return {
        exports,
        requireModule,
        _modulesRegistry: modulesRegistry,
        _jsonpCallback: jsonpCallback,
        _makoModuleHotUpdate: requireModule.applyHotUpdate
    };
}
const runtime = createRuntime(m, e);
(typeof globalThis !== 'undefined' ? globalThis : self).jsonpCallback = runtime._jsonpCallback;
(typeof globalThis !== 'undefined' ? globalThis : self).modulesRegistry = runtime._modulesRegistry;
(typeof globalThis !== 'undefined' ? globalThis : self).makoModuleHotUpdate = runtime._makoModuleHotUpdate;

//# sourceMappingURL=index.js.map