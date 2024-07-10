use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use glob::glob;
use path_clean::PathClean;
use pathdiff::diff_paths;
use regex::RegexBuilder;

use super::param::ContextLoadMode;
use crate::compiler::Context;
use crate::module::ModuleId;

pub struct VirtualContextModuleRender {
    mode: ContextLoadMode,
    root: String,
    reg: String,
    ignore_case: bool,
    sub_directories: bool,
}

impl ContextLoadMode {
    fn render_module_import(&self, map: &BTreeMap<String, String>) -> String {
        let mut map_str = String::from(r#"var _map_lazy = {"#);
        map.iter().for_each(|(key, value)| match self {
            ContextLoadMode::Sync => {
                map_str.push_str(&format!(
                    r#"
  "{}": ()=> require("{}"),"#,
                    key, value
                ));
            }

            ContextLoadMode::Lazy => {
                map_str.push_str(&format!(
                    r#"
  "{}": ()=> import("{}"),"#,
                    key, value
                ));
            }
            ContextLoadMode::Eager | ContextLoadMode::Weak | ContextLoadMode::LazyOnce => {
                unimplemented!("{} Mode not implemented", self);
            }
        });

        map_str.push_str("\n};\n");
        map_str
    }

    fn module_context(self) -> String {
        match self {
            ContextLoadMode::Sync => r#"
  module.exports = function contextRequire(req){{
    var call  = _map_lazy[req];
    if(call){{
      return call()
    }}else{{
      var e = new Error("Cannot find module '" + req + "'");
	  e.code = 'MODULE_NOT_FOUND';
	  throw e;
    }}
  }};
"#
            .to_string(),
            ContextLoadMode::Lazy => r#"
  module.exports = function contextRequire(req){{
    var call  = _map_lazy[req];
    if(call){{
      return call()
    }}else{{
      var e = new Error("Cannot find module '" + req + "'");
      e.code = 'MODULE_NOT_FOUND';
      return Promise.reject(e)
    }} 
  }};  
"#
            .to_string(),
            ContextLoadMode::Eager | ContextLoadMode::Weak | ContextLoadMode::LazyOnce => {
                unimplemented!("{} Mode not implemented", self)
            }
        }
    }
}

impl VirtualContextModuleRender {
    fn id(&self) -> String {
        // TODO align with webpack
        format!(
            "./{}/ {} {}{}/",
            self.root,
            self.mode,
            if self.sub_directories {
                ""
            } else {
                "nonrecursive "
            },
            &self.reg,
        )
    }

    fn matched_files(&self, context: &Arc<Context>) -> Result<BTreeMap<String, String>> {
        let root_path = context
            .root
            .join(&self.root)
            .clean()
            .to_string_lossy()
            .to_string();

        if self.root.starts_with("../") {
            return Err(anyhow!(
                "Context Module Using Invalid path: `{}` , reference file out of current project \
                root not allowed.",
                root_path
            ));
        }

        let glob_str = if self.sub_directories {
            format!("{}/**/*.*", root_path)
        } else {
            format!("{}/*.*", root_path)
        };

        let glob = glob(&glob_str)?;

        let mut source_to_path = BTreeMap::new();

        let regex = RegexBuilder::new(&self.reg)
            .case_insensitive(self.ignore_case)
            .build()?;

        for matched in glob.filter_map(Result::ok) {
            if let Some(p) = diff_paths(&matched, &root_path) {
                let mut source = p.to_string_lossy().to_string();
                if !source.starts_with('.') {
                    source.insert_str(0, "./");
                }

                if regex.is_match(&source) {
                    source_to_path.insert(source, matched.to_string_lossy().to_string());
                }
            }
        }

        Ok(source_to_path)
    }

    pub fn module_id_map(&self, map: &BTreeMap<String, String>, context: &Arc<Context>) -> String {
        let mut map_str = String::from(r#"var _map = {"#);
        for (key, value) in map.iter() {
            map_str.push_str(&format!(
                r#"
  "{}": "{}","#,
                key,
                ModuleId::from(value.as_str()).generate(context)
            ));
        }
        map_str.push_str("\n};\n");

        map_str
    }

    pub fn module_import(&self, map: &BTreeMap<String, String>) -> String {
        self.mode.render_module_import(map)
    }

    pub fn module_context(&self) -> String {
        self.mode.module_context()
    }

    pub fn render(&self, context: Arc<Context>) -> Result<String> {
        let source_to_path = self.matched_files(&context)?;
        let id = self.id();

        Ok(format!(
            r#"
// context Map
{}
// context lazy require function Map 
{}
// context require
{}
module.exports.resolve  = function(req) {{
    var r = _map[req];
    if(r){{
        return r
    }}else{{    
      var e = new Error("Cannot find module '" + req + "'");
	  e.code = 'MODULE_NOT_FOUND';
	  throw e;    
    }}
}};

module.exports.keys = function() {{ return Object.keys(_map); }}       
            
module.exports.id = "{id}";            
"#,
            self.module_id_map(&source_to_path, &context),
            self.module_import(&source_to_path),
            self.module_context(),
        ))
    }
}

impl TryFrom<HashMap<String, String>> for VirtualContextModuleRender {
    type Error = anyhow::Error;

    fn try_from(value: HashMap<String, String>) -> Result<Self, Self::Error> {
        let invalid: bool = value
            .get("invalid")
            .map_or(false, |i| i.parse::<bool>().unwrap_or(false));

        if invalid {
            return Err(anyhow!("Invalid"));
        }

        let mode = value.get("mode");
        let reg = value.get("reg");
        let root = value.get("root");
        let sub = value.get("sub");
        let ig = value.get("ig");

        match (mode, reg, root, sub, ig) {
            (Some(mode), Some(reg), Some(root), Some(sub), Some(ig)) => {
                let m: ContextLoadMode = mode.try_into().unwrap_or(ContextLoadMode::Sync);
                let sub_dir: bool = sub.parse().unwrap_or(false);
                let ig: bool = ig.parse().unwrap_or(false);

                Ok(VirtualContextModuleRender {
                    sub_directories: sub_dir,
                    mode: m,
                    root: root.clone(),
                    reg: reg.clone(),
                    ignore_case: ig,
                })
            }
            _ => Err(anyhow!("Invalid")),
        }
    }
}
