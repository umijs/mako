globalThis.jsonpCallback([
    [
        "chunk-1.ts"
    ],
    {
        "chunk-1.ts": function(module, exports, require) {
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            Object.defineProperty(exports, "default", {
                enumerable: true,
                get: function() {
                    return _default;
                }
            });
            async function _default() {
                console.log(123);
            }
        }
    }
]);

//# sourceMappingURL=chunk-1_ts-async.js.map