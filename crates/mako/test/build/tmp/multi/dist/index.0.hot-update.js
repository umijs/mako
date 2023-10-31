globalThis.makoModuleHotUpdate('index.ts', {
    modules: {
        "index.ts": function(module, exports, require) {
            (async ()=>{
                await Promise.all([
                    require.ensure("chunk-2.ts")
                ]).then(require.bind(require, "chunk-2.ts"));
            })();
        }
    }
}, function(runtime) {
    runtime._h = '13484964391977451483';
    ;
});

//# sourceMappingURL=index.0.hot-update.js.map