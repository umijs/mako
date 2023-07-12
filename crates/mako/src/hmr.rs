use std::collections::HashSet;

use anyhow::Result;
use swc_ecma_ast::{
    CallExpr, Expr, ExprOrSpread, ExprStmt, KeyValueProp, ModuleItem, ObjectLit, Prop,
    PropOrSpread, Stmt,
};

use crate::ast::{build_js_ast, js_ast_to_code};
use crate::chunk::Chunk;
use crate::compiler::Compiler;
use crate::generate_chunks::modules_to_js_stmts;
use crate::module::ModuleId;

impl Compiler {
    pub fn generate_hmr_chunk(
        &self,
        chunk: &Chunk,
        module_ids: &HashSet<ModuleId>,
        current_hash: u64,
    ) -> Result<(String, String)> {
        let module_graph = &self.context.module_graph.read().unwrap();
        let (js_stmts, _) = modules_to_js_stmts(module_ids, module_graph, &self.context);
        let mut content = include_str!("runtime/runtime_hmr.js").to_string();
        content = content
            .replace("__CHUNK_ID__", &chunk.id.generate(&self.context))
            .replace(
                "__runtime_code__",
                &format!("runtime._h='{}';", current_hash),
            );
        let filename = &chunk.filename();
        // TODO: handle error
        let mut js_ast = build_js_ast(filename, content.as_str(), &self.context)
            .unwrap()
            .ast;

        for stmt in &mut js_ast.body {
            if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
                expr: box Expr::Call(CallExpr { args, .. }),
                ..
            })) = stmt
            {
                if let ExprOrSpread {
                    expr: box Expr::Object(ObjectLit { props, .. }),
                    ..
                } = &mut args[1]
                {
                    if props.len() != 1 {
                        panic!("hmr runtime should only have one modules property");
                    }
                    if let PropOrSpread::Prop(box Prop::KeyValue(KeyValueProp {
                        value: box Expr::Object(ObjectLit { props, .. }),
                        ..
                    })) = &mut props[0]
                    {
                        props.extend(js_stmts);
                        break;
                    }
                }
            }
        }

        let (js_code, js_sourcemap) = js_ast_to_code(&js_ast, &self.context, filename).unwrap();
        Ok((js_code, js_sourcemap))
    }
}

#[cfg(test)]
mod tests {

    use crate::compiler::Compiler;
    use crate::config::Config;
    use crate::transform_in_generate::transform_modules;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_generate_hmr_chunk() {
        let compiler = create_compiler("test/dev/normal");

        compiler.build();
        compiler.group_chunk();
        let chunk_graph = &compiler.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();
        let chunk = chunks[0];
        let module_ids = chunk.get_modules();
        transform_modules(module_ids.iter().cloned().collect(), &compiler.context).unwrap();
        let (js_code, _js_sourcemap) = compiler.generate_hmr_chunk(chunk, module_ids, 42).unwrap();
        let js_code = js_code.replace(
            compiler.context.root.to_string_lossy().to_string().as_str(),
            "",
        );
        println!("{}", js_code);
        assert_eq!(
            js_code.trim(),
            r#"
globalThis.makoModuleHotUpdate('./index.ts', {
    modules: {
        "./bar_1.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("./foo.ts");
        },
        "./bar_2.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("./foo.ts");
        },
        "./foo.ts": function(module, exports, require) {
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            Object.defineProperty(exports, "default", {
                enumerable: true,
                get: function() {
                    return _default;
                }
            });
            var _default = 1;
        },
        "./index.ts": function(module, exports, require) {
            const RefreshRuntime = require("../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/runtime.js");
            RefreshRuntime.injectIntoGlobalHook(window);
            window.$RefreshReg$ = ()=>{};
            window.$RefreshSig$ = ()=>(type)=>type;
            "use strict";
            Object.defineProperty(exports, "__esModule", {
                value: true
            });
            require("../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/runtime.js");
            require("./bar_1.ts");
            require("./bar_2.ts");
            require("./hoo");
            (function() {
                const socket = new WebSocket('ws://127.0.0.1:3000/__/hmr-ws');
                let latestHash = '';
                let updating = false;
                function runHotUpdate() {
                    if (latestHash !== require.currentHash()) {
                        updating = true;
                        return module.hot.check().then(()=>{
                            updating = false;
                            return runHotUpdate();
                        }).catch((e)=>{
                            console.error('[HMR] HMR check failed', e);
                        });
                    } else {
                        return Promise.resolve();
                    }
                }
                socket.addEventListener('message', (rawMessage)=>{
                    console.log(rawMessage);
                    const msg = JSON.parse(rawMessage.data);
                    latestHash = msg.hash;
                    if (!updating) {
                        runHotUpdate();
                    }
                });
            })();
        },
        "../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/cjs/react-refresh-runtime.development.js": function(module, exports, require) {
            'use strict';
            {
                (function() {
                    'use strict';
                    var REACT_FORWARD_REF_TYPE = Symbol.for('react.forward_ref');
                    var REACT_MEMO_TYPE = Symbol.for('react.memo');
                    var PossiblyWeakMap = typeof WeakMap === 'function' ? WeakMap : Map;
                    var allFamiliesByID = new Map();
                    var allFamiliesByType = new PossiblyWeakMap();
                    var allSignaturesByType = new PossiblyWeakMap();
                    var updatedFamiliesByType = new PossiblyWeakMap();
                    var pendingUpdates = [];
                    var helpersByRendererID = new Map();
                    var helpersByRoot = new Map();
                    var mountedRoots = new Set();
                    var failedRoots = new Set();
                    var rootElements = typeof WeakMap === 'function' ? new WeakMap() : null;
                    var isPerformingRefresh = false;
                    function computeFullKey(signature) {
                        if (signature.fullKey !== null) {
                            return signature.fullKey;
                        }
                        var fullKey = signature.ownKey;
                        var hooks;
                        try {
                            hooks = signature.getCustomHooks();
                        } catch (err) {
                            signature.forceReset = true;
                            signature.fullKey = fullKey;
                            return fullKey;
                        }
                        for(var i = 0; i < hooks.length; i++){
                            var hook = hooks[i];
                            if (typeof hook !== 'function') {
                                signature.forceReset = true;
                                signature.fullKey = fullKey;
                                return fullKey;
                            }
                            var nestedHookSignature = allSignaturesByType.get(hook);
                            if (nestedHookSignature === undefined) {
                                continue;
                            }
                            var nestedHookKey = computeFullKey(nestedHookSignature);
                            if (nestedHookSignature.forceReset) {
                                signature.forceReset = true;
                            }
                            fullKey += '\n---\n' + nestedHookKey;
                        }
                        signature.fullKey = fullKey;
                        return fullKey;
                    }
                    function haveEqualSignatures(prevType, nextType) {
                        var prevSignature = allSignaturesByType.get(prevType);
                        var nextSignature = allSignaturesByType.get(nextType);
                        if (prevSignature === undefined && nextSignature === undefined) {
                            return true;
                        }
                        if (prevSignature === undefined || nextSignature === undefined) {
                            return false;
                        }
                        if (computeFullKey(prevSignature) !== computeFullKey(nextSignature)) {
                            return false;
                        }
                        if (nextSignature.forceReset) {
                            return false;
                        }
                        return true;
                    }
                    function isReactClass(type) {
                        return type.prototype && type.prototype.isReactComponent;
                    }
                    function canPreserveStateBetween(prevType, nextType) {
                        if (isReactClass(prevType) || isReactClass(nextType)) {
                            return false;
                        }
                        if (haveEqualSignatures(prevType, nextType)) {
                            return true;
                        }
                        return false;
                    }
                    function resolveFamily(type) {
                        return updatedFamiliesByType.get(type);
                    }
                    function cloneMap(map) {
                        var clone = new Map();
                        map.forEach(function(value, key) {
                            clone.set(key, value);
                        });
                        return clone;
                    }
                    function cloneSet(set) {
                        var clone = new Set();
                        set.forEach(function(value) {
                            clone.add(value);
                        });
                        return clone;
                    }
                    function getProperty(object, property) {
                        try {
                            return object[property];
                        } catch (err) {
                            return undefined;
                        }
                    }
                    function performReactRefresh() {
                        if (pendingUpdates.length === 0) {
                            return null;
                        }
                        if (isPerformingRefresh) {
                            return null;
                        }
                        isPerformingRefresh = true;
                        try {
                            var staleFamilies = new Set();
                            var updatedFamilies = new Set();
                            var updates = pendingUpdates;
                            pendingUpdates = [];
                            updates.forEach(function(_ref) {
                                var family = _ref[0], nextType = _ref[1];
                                var prevType = family.current;
                                updatedFamiliesByType.set(prevType, family);
                                updatedFamiliesByType.set(nextType, family);
                                family.current = nextType;
                                if (canPreserveStateBetween(prevType, nextType)) {
                                    updatedFamilies.add(family);
                                } else {
                                    staleFamilies.add(family);
                                }
                            });
                            var update = {
                                updatedFamilies: updatedFamilies,
                                staleFamilies: staleFamilies
                            };
                            helpersByRendererID.forEach(function(helpers) {
                                helpers.setRefreshHandler(resolveFamily);
                            });
                            var didError = false;
                            var firstError = null;
                            var failedRootsSnapshot = cloneSet(failedRoots);
                            var mountedRootsSnapshot = cloneSet(mountedRoots);
                            var helpersByRootSnapshot = cloneMap(helpersByRoot);
                            failedRootsSnapshot.forEach(function(root) {
                                var helpers = helpersByRootSnapshot.get(root);
                                if (helpers === undefined) {
                                    throw new Error('Could not find helpers for a root. This is a bug in React Refresh.');
                                }
                                if (!failedRoots.has(root)) {}
                                if (rootElements === null) {
                                    return;
                                }
                                if (!rootElements.has(root)) {
                                    return;
                                }
                                var element = rootElements.get(root);
                                try {
                                    helpers.scheduleRoot(root, element);
                                } catch (err) {
                                    if (!didError) {
                                        didError = true;
                                        firstError = err;
                                    }
                                }
                            });
                            mountedRootsSnapshot.forEach(function(root) {
                                var helpers = helpersByRootSnapshot.get(root);
                                if (helpers === undefined) {
                                    throw new Error('Could not find helpers for a root. This is a bug in React Refresh.');
                                }
                                if (!mountedRoots.has(root)) {}
                                try {
                                    helpers.scheduleRefresh(root, update);
                                } catch (err) {
                                    if (!didError) {
                                        didError = true;
                                        firstError = err;
                                    }
                                }
                            });
                            if (didError) {
                                throw firstError;
                            }
                            return update;
                        } finally{
                            isPerformingRefresh = false;
                        }
                    }
                    function register(type, id) {
                        {
                            if (type === null) {
                                return;
                            }
                            if (typeof type !== 'function' && typeof type !== 'object') {
                                return;
                            }
                            if (allFamiliesByType.has(type)) {
                                return;
                            }
                            var family = allFamiliesByID.get(id);
                            if (family === undefined) {
                                family = {
                                    current: type
                                };
                                allFamiliesByID.set(id, family);
                            } else {
                                pendingUpdates.push([
                                    family,
                                    type
                                ]);
                            }
                            allFamiliesByType.set(type, family);
                            if (typeof type === 'object' && type !== null) {
                                switch(getProperty(type, '$$typeof')){
                                    case REACT_FORWARD_REF_TYPE:
                                        register(type.render, id + '$render');
                                        break;
                                    case REACT_MEMO_TYPE:
                                        register(type.type, id + '$type');
                                        break;
                                }
                            }
                        }
                    }
                    function setSignature(type, key) {
                        var forceReset = arguments.length > 2 && arguments[2] !== undefined ? arguments[2] : false;
                        var getCustomHooks = arguments.length > 3 ? arguments[3] : undefined;
                        {
                            if (!allSignaturesByType.has(type)) {
                                allSignaturesByType.set(type, {
                                    forceReset: forceReset,
                                    ownKey: key,
                                    fullKey: null,
                                    getCustomHooks: getCustomHooks || function() {
                                        return [];
                                    }
                                });
                            }
                            if (typeof type === 'object' && type !== null) {
                                switch(getProperty(type, '$$typeof')){
                                    case REACT_FORWARD_REF_TYPE:
                                        setSignature(type.render, key, forceReset, getCustomHooks);
                                        break;
                                    case REACT_MEMO_TYPE:
                                        setSignature(type.type, key, forceReset, getCustomHooks);
                                        break;
                                }
                            }
                        }
                    }
                    function collectCustomHooksForSignature(type) {
                        {
                            var signature = allSignaturesByType.get(type);
                            if (signature !== undefined) {
                                computeFullKey(signature);
                            }
                        }
                    }
                    function getFamilyByID(id) {
                        {
                            return allFamiliesByID.get(id);
                        }
                    }
                    function getFamilyByType(type) {
                        {
                            return allFamiliesByType.get(type);
                        }
                    }
                    function findAffectedHostInstances(families) {
                        {
                            var affectedInstances = new Set();
                            mountedRoots.forEach(function(root) {
                                var helpers = helpersByRoot.get(root);
                                if (helpers === undefined) {
                                    throw new Error('Could not find helpers for a root. This is a bug in React Refresh.');
                                }
                                var instancesForRoot = helpers.findHostInstancesForRefresh(root, families);
                                instancesForRoot.forEach(function(inst) {
                                    affectedInstances.add(inst);
                                });
                            });
                            return affectedInstances;
                        }
                    }
                    function injectIntoGlobalHook(globalObject) {
                        {
                            var hook = globalObject.__REACT_DEVTOOLS_GLOBAL_HOOK__;
                            if (hook === undefined) {
                                var nextID = 0;
                                globalObject.__REACT_DEVTOOLS_GLOBAL_HOOK__ = hook = {
                                    renderers: new Map(),
                                    supportsFiber: true,
                                    inject: function(injected) {
                                        return nextID++;
                                    },
                                    onScheduleFiberRoot: function(id, root, children) {},
                                    onCommitFiberRoot: function(id, root, maybePriorityLevel, didError) {},
                                    onCommitFiberUnmount: function() {}
                                };
                            }
                            if (hook.isDisabled) {
                                console['warn']('Something has shimmed the React DevTools global hook (__REACT_DEVTOOLS_GLOBAL_HOOK__). ' + 'Fast Refresh is not compatible with this shim and will be disabled.');
                                return;
                            }
                            var oldInject = hook.inject;
                            hook.inject = function(injected) {
                                var id = oldInject.apply(this, arguments);
                                if (typeof injected.scheduleRefresh === 'function' && typeof injected.setRefreshHandler === 'function') {
                                    helpersByRendererID.set(id, injected);
                                }
                                return id;
                            };
                            hook.renderers.forEach(function(injected, id) {
                                if (typeof injected.scheduleRefresh === 'function' && typeof injected.setRefreshHandler === 'function') {
                                    helpersByRendererID.set(id, injected);
                                }
                            });
                            var oldOnCommitFiberRoot = hook.onCommitFiberRoot;
                            var oldOnScheduleFiberRoot = hook.onScheduleFiberRoot || function() {};
                            hook.onScheduleFiberRoot = function(id, root, children) {
                                if (!isPerformingRefresh) {
                                    failedRoots.delete(root);
                                    if (rootElements !== null) {
                                        rootElements.set(root, children);
                                    }
                                }
                                return oldOnScheduleFiberRoot.apply(this, arguments);
                            };
                            hook.onCommitFiberRoot = function(id, root, maybePriorityLevel, didError) {
                                var helpers = helpersByRendererID.get(id);
                                if (helpers !== undefined) {
                                    helpersByRoot.set(root, helpers);
                                    var current = root.current;
                                    var alternate = current.alternate;
                                    if (alternate !== null) {
                                        var wasMounted = alternate.memoizedState != null && alternate.memoizedState.element != null && mountedRoots.has(root);
                                        var isMounted = current.memoizedState != null && current.memoizedState.element != null;
                                        if (!wasMounted && isMounted) {
                                            mountedRoots.add(root);
                                            failedRoots.delete(root);
                                        } else if (wasMounted && isMounted) ;
                                        else if (wasMounted && !isMounted) {
                                            mountedRoots.delete(root);
                                            if (didError) {
                                                failedRoots.add(root);
                                            } else {
                                                helpersByRoot.delete(root);
                                            }
                                        } else if (!wasMounted && !isMounted) {
                                            if (didError) {
                                                failedRoots.add(root);
                                            }
                                        }
                                    } else {
                                        mountedRoots.add(root);
                                    }
                                }
                                return oldOnCommitFiberRoot.apply(this, arguments);
                            };
                        }
                    }
                    function hasUnrecoverableErrors() {
                        return false;
                    }
                    function _getMountedRootCount() {
                        {
                            return mountedRoots.size;
                        }
                    }
                    function createSignatureFunctionForTransform() {
                        {
                            var savedType;
                            var hasCustomHooks;
                            var didCollectHooks = false;
                            return function(type, key, forceReset, getCustomHooks) {
                                if (typeof key === 'string') {
                                    if (!savedType) {
                                        savedType = type;
                                        hasCustomHooks = typeof getCustomHooks === 'function';
                                    }
                                    if (type != null && (typeof type === 'function' || typeof type === 'object')) {
                                        setSignature(type, key, forceReset, getCustomHooks);
                                    }
                                    return type;
                                } else {
                                    if (!didCollectHooks && hasCustomHooks) {
                                        didCollectHooks = true;
                                        collectCustomHooksForSignature(savedType);
                                    }
                                }
                            };
                        }
                    }
                    function isLikelyComponentType(type) {
                        {
                            switch(typeof type){
                                case 'function':
                                    {
                                        if (type.prototype != null) {
                                            if (type.prototype.isReactComponent) {
                                                return true;
                                            }
                                            var ownNames = Object.getOwnPropertyNames(type.prototype);
                                            if (ownNames.length > 1 || ownNames[0] !== 'constructor') {
                                                return false;
                                            }
                                            if (type.prototype.__proto__ !== Object.prototype) {
                                                return false;
                                            }
                                        }
                                        var name = type.name || type.displayName;
                                        return typeof name === 'string' && /^[A-Z]/.test(name);
                                    }
                                case 'object':
                                    {
                                        if (type != null) {
                                            switch(getProperty(type, '$$typeof')){
                                                case REACT_FORWARD_REF_TYPE:
                                                case REACT_MEMO_TYPE:
                                                    return true;
                                                default:
                                                    return false;
                                            }
                                        }
                                        return false;
                                    }
                                default:
                                    {
                                        return false;
                                    }
                            }
                        }
                    }
                    exports._getMountedRootCount = _getMountedRootCount;
                    exports.collectCustomHooksForSignature = collectCustomHooksForSignature;
                    exports.createSignatureFunctionForTransform = createSignatureFunctionForTransform;
                    exports.findAffectedHostInstances = findAffectedHostInstances;
                    exports.getFamilyByID = getFamilyByID;
                    exports.getFamilyByType = getFamilyByType;
                    exports.hasUnrecoverableErrors = hasUnrecoverableErrors;
                    exports.injectIntoGlobalHook = injectIntoGlobalHook;
                    exports.isLikelyComponentType = isLikelyComponentType;
                    exports.performReactRefresh = performReactRefresh;
                    exports.register = register;
                    exports.setSignature = setSignature;
                })();
            }
        },
        "../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/runtime.js": function(module, exports, require) {
            'use strict';
            {
                module.exports = require("../../../../../node_modules/.pnpm/react-refresh@0.14.0/node_modules/react-refresh/cjs/react-refresh-runtime.development.js");
            }
        },
        "./hoo": function(module, exports, require) {
            "use strict";
            module.exports = hoo;
        }
    }
}, function(runtime) {
    runtime._h = '42';
    ;
});

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn create_compiler(base: &str) -> Compiler {
        let current_dir = std::env::current_dir().unwrap();
        let root = current_dir.join(base);
        let config = Config::new(&root, None, None).unwrap();
        Compiler::new(config, root)
    }
}
