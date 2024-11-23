use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tracing::warn;

use crate::ast::file::Content;
use crate::compiler::Context;
use crate::config::ModuleFederationConfig;
use crate::module::md5_hash;
use crate::plugin::Plugin;
use crate::visitors::mako_require::MAKO_REQUIRE;

pub struct ModuleFederationPlugin {
    pub config: ModuleFederationConfig,
}

const FEDERATION_GLOBAL: &str = "__mako_require__.federation";

impl ModuleFederationPlugin {
    pub fn new(config: ModuleFederationConfig) -> Self {
        Self { config }
    }
}

impl Plugin for ModuleFederationPlugin {
    fn name(&self) -> &str {
        "module_federation"
    }

    fn modify_config(
        &self,
        config: &mut crate::config::Config,
        root: &std::path::Path,
        _args: &crate::compiler::Args,
    ) -> Result<()> {
        if let Some(exposes) = self.config.exposes.as_ref() {
            for (name, import) in exposes.iter() {
                match config.entry.entry(name.to_string()) {
                    Occupied(_) => {
                        warn!("mf exposed name {} is duplcated with entry config.", name);
                    }
                    Vacant(vacant_entry) => {
                        if let Ok(entry_path) = root.join(import).canonicalize() {
                            vacant_entry.insert(entry_path);
                        } else {
                            return Err(anyhow!("mf exposed file :{} not found", import));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn load_transform(
        &self,
        _content: &mut Content,
        _path: &str,
        _is_entry: bool,
        _context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        if !_is_entry {
            Ok(None)
        } else {
            match _content {
                Content::Js(js_content) => {
                    let entry_runtime_dep_path = self.prepare_entry_runtime_dep(&_context.root);
                    js_content.content.insert_str(
                        0,
                        format!(r#"import "{}";"#, entry_runtime_dep_path).as_str(),
                    );
                    Ok(Some(_content.clone()))
                }
                _ => Ok(None),
            }
        }
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> Result<Vec<String>> {
        let code = r#"
/* mako/runtime/federation runtime */
!(function() {
  if(!requireModule.federation) {
    requireModule.federation = {
      initOptions: {},
      chunkMatcher: undefined,
      rootOutputDir: "",
      initialConsumes: undefined,
      bundlerRuntimeOptions: {}
    };
  }
})();"#
            .to_string();
        Ok(vec![code])
    }
}

impl ModuleFederationPlugin {
    fn prepare_entry_runtime_dep(&self, root: &Path) -> String {
        let entry_runtime_code = self.get_entry_runtime_code();

        let content_hash = md5_hash(&entry_runtime_code, 32);
        let dep_path = root.join(format!("node_modules/.entry.{}.js", content_hash));
        let dep_parent_path = dep_path.parent().unwrap();
        if !fs::exists(dep_parent_path).unwrap() {
            fs::create_dir(dep_parent_path).unwrap();
        }
        if !fs::exists(&dep_path).unwrap() {
            fs::write(&dep_path, entry_runtime_code).unwrap();
        }

        dep_path.to_string_lossy().to_string()
    }

    fn get_entry_runtime_code(&self) -> String {
        let embed_runtime_codes = format!(
            r#"if(!{federation_global}.runtime) {{
  var preFederation = {federation_global};
  {federation_global} = {{}};
  for(var key in federation) {{
    {federation_global}[key] = federation[key];
  }}
  for(var key in preFederation) {{
    {federation_global}[key] = preFederation[key];
  }}
}}"#,
            federation_global = FEDERATION_GLOBAL
        );

        let (imported_plugin_names, import_plugin_stmts) =
            self.config.runtime_plugins.iter().enumerate().fold(
                (Vec::new(), Vec::new()),
                |(mut names, mut stmts), (plugin, index)| {
                    names.push(format!("plugin_{}", index));
                    stmts.push(format!(r#"import plugin_{} from "{}""#, index, plugin));
                    (names, stmts)
                },
            );

        let plugins_imports = import_plugin_stmts.join(";");

        let plugins_collection = if imported_plugin_names.is_empty() {
            "".to_string()
        } else {
            format!(
                r#"var pluginsToAdd = [{plugins_to_add}].filter(Boolean);
  {federation_global}.initOptions.plugins = {federation_global}.initOptions.plugins ?
    {federation_global}.initOptions.plugins.concat(pluginsToAdd) : pluginsToAdd;
"#,
                plugins_to_add = imported_plugin_names
                    .iter()
                    .map(|item| format!(r#"{item} ? (item.default || item)() : false"#))
                    .collect::<Vec<_>>()
                    .join(","),
                federation_global = FEDERATION_GLOBAL
            )
        };

        format!(
            r#"import federation from "{federation_impl}";
{plugins_imports}
{embed_runtime_codes}

if(!{federation_global}.instance) {{
  {plugins_collection}
  {federation_global}.instance = {federation_global}.runtime.init({federation_global}.initOptions);
  if({federation_global}.attachShareScopeMap) {{
    {federation_global}.attachShareScopeMap({mako_require});
  }}
  if({federation_global}.installInitialConsumes) {{
    {federation_global}.installInitialConsumes();
  }}
}}
"#,
            embed_runtime_codes = embed_runtime_codes,
            plugins_imports = plugins_imports,
            plugins_collection = plugins_collection,
            federation_impl = self.config.implementation,
            federation_global = FEDERATION_GLOBAL,
            mako_require = MAKO_REQUIRE
        )
    }
}
