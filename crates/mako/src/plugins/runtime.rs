use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};

use crate::compiler::Context;
use crate::generate::swc_helpers::SwcHelpers;
use crate::module::ModuleId;
use crate::plugin::Plugin;

pub struct MakoRuntime {}

impl Plugin for MakoRuntime {
    fn name(&self) -> &str {
        "mako/runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let plugins = vec![
            self.public_path(context),
            self.helper_runtime(context).unwrap(),
        ];
        Ok(plugins)
    }
}

impl MakoRuntime {
    fn public_path(&self, context: &Arc<Context>) -> String {
        let public_path = context.config.public_path.clone();
        let public_path = if public_path == "runtime" {
            "(typeof globalThis !== 'undefined' ? globalThis : self).publicPath || '/'".to_string()
        } else {
            format!("\"{}\"", public_path)
        };

        format!(
            r#"
  /* mako/runtime/publicPath */
  !function () {{
    requireModule.publicPath= {};
  }}();"#,
            public_path
        )
    }

    fn helper_runtime(&self, context: &Arc<Context>) -> Result<String> {
        let helpers = SwcHelpers::full_helpers()
            .into_iter()
            .map(|source| {
                let code = Self::get_swc_helper_code(&source).unwrap();
                let module_id: ModuleId = source.into();
                let module_id = module_id.generate(context);
                format!("\"{}\": {}", module_id, code)
            })
            .collect::<Vec<_>>()
            .join(",\n");

        Ok(format!(
            r#"
  /* mako/runtime/helpers */
  registerModules({{
    {}
  }});
        "#,
            helpers
        ))
    }

    fn get_swc_helper_code(path: &str) -> Result<String> {
        let code = match path {
            "@swc/helpers/_/_interop_require_default" => r#"
function(module, exports, __mako_require__) {
    __mako_require__.d(exports, "__esModule", {
        value: true
    });
    function _export(target, all) {
        for(var name in all)Object.defineProperty(target, name, {
            enumerable: true,
            get: all[name]
        });
    }
    __mako_require__.e(exports, {
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
}
            "#.trim(),
            "@swc/helpers/_/_interop_require_wildcard" => r#"
function(module, exports, __mako_require__) {
    __mako_require__.d(exports, "__esModule", {
        value: true
    });
    function _export(target, all) {
        for(var name in all)Object.defineProperty(target, name, {
            enumerable: true,
            get: all[name]
        });
    }
    __mako_require__.e(exports, {
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
        for(var key in obj)if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) Object.defineProperty(newObj, key, desc);
            else newObj[key] = obj[key];
        }
        newObj.default = obj;
        if (cache) cache.set(obj, newObj);
        return newObj;
    }
}
            "#.trim(),
            "@swc/helpers/_/_export_star" => r#"
function(module, exports, __mako_require__) {
    __mako_require__.d(exports, "__esModule", {
        value: true
    });
    function _export(target, all) {
        for(var name in all)Object.defineProperty(target, name, {
            enumerable: true,
            get: all[name]
        });
    }
    __mako_require__.e(exports, {
        _export_star: function() {
            return _export_star;
        },
        _: function() {
            return _export_star;
        }
    });
    function _export_star(from, to) {
        Object.keys(from).forEach(function(k) {
            if (k !== "default" && !Object.prototype.hasOwnProperty.call(to, k)) Object.defineProperty(to, k, {
                enumerable: true,
                get: function() {
                    return from[k];
                }
            });
        });
        return from;
    }
}
            "#.trim(),
            _ => return Err(anyhow!("swc helper not found: {}", path)),
        };
        Ok(code.to_string())
    }
}
