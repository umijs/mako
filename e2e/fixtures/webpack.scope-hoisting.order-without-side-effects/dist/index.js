!(function() {
    var chunksIdToUrlMap = {};
    var cssChunksIdToUrlMap = {};
    var e = "index.js";
    var cssInstalledChunks = {
        "index.js": 0
    };
    var m = {
        /*./index.js*/ "index.js": function(module, exports, __mako_require__) {
            var array = [];
            array.push("a");
            var a = 1;
            array.push("b");
            var b = 2;
            it("should concatenate in correct order", function() {
                expect(b).toBe(2);
                expect(a).toBe(1);
                expect(array).toEqual([
                    "a",
                    "b"
                ]);
            });
        }
    };
    function createRuntime(makoModules, entryModuleId) {
        var global = typeof globalThis !== 'undefined' ? globalThis : self;
        var modulesRegistry = {};
        function requireModule(moduleId) {
            if (moduleId === '$$IGNORED$$') return {};
            var cachedModule = modulesRegistry[moduleId];
            if (cachedModule !== undefined) {
                return cachedModule.exports;
            }
            var module = {
                id: moduleId,
                exports: {}
            };
            modulesRegistry[moduleId] = module;
            var execOptions = {
                id: moduleId,
                module: module,
                factory: makoModules[moduleId],
                require: requireModule
            };
            requireModule.requireInterceptors.forEach(function(interceptor) {
                interceptor(execOptions);
            });
            execOptions.factory.call(execOptions.module.exports, execOptions.module, execOptions.module.exports, execOptions.require);
            return module.exports;
        }
        // module execution interceptor
        requireModule.requireInterceptors = [];
        // module utils
        requireModule.e = function(target, all) {
            for(var name in all)Object.defineProperty(target, name, {
                enumerable: true,
                get: all[name]
            });
        };
        requireModule.d = Object.defineProperty.bind(Object);
        var registerModules = function(modules) {
            for(var id in modules){
                makoModules[id] = modules[id];
            }
        };
        /* mako/runtime/publicPath */ !function() {
            requireModule.publicPath = "/";
        }();
        requireModule(entryModuleId);
        return {
            requireModule: requireModule
        };
    }
    createRuntime(m, e);
})();

//# sourceMappingURL=index.js.map