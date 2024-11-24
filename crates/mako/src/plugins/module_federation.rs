use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tracing::warn;

use crate::ast::file::Content;
use crate::compiler::{Args, Context};
use crate::config::module_federation::ModuleFederationConfig;
use crate::config::Config;
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

    fn modify_config(&self, config: &mut Config, root: &Path, _args: &Args) -> Result<()> {
        if let Some(exposes) = self.config.exposes.as_ref() {
            let container_entry_name = &self.config.name;
            if !exposes.is_empty() {
                match config.entry.entry(container_entry_name.clone()) {
                    Occupied(_) => {
                        warn!(
                            "mf exposed name {} is duplcated with entry config.",
                            container_entry_name
                        );
                    }
                    Vacant(vacant_entry) => {
                        let container_entry_code = self.get_container_entry_code(root);
                        let container_entry_path = root.join(format!(
                            "node_modules/.federation/.entry.container.{}.js",
                            container_entry_name
                        ));
                        let container_entry_parent_path = container_entry_path.parent().unwrap();
                        if !fs::exists(container_entry_parent_path).unwrap() {
                            fs::create_dir(container_entry_parent_path).unwrap();
                        }
                        fs::write(&container_entry_path, container_entry_code).unwrap();

                        vacant_entry.insert(container_entry_path);
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
                        format!(
                            r#"import "{}";
"#,
                            entry_runtime_dep_path
                        )
                        .as_str(),
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

        let dep_path = root.join(format!(
            "node_modules/.federation/.entry.{}.js",
            content_hash
        ));
        let dep_parent_path = dep_path.parent().unwrap();
        if !fs::exists(dep_parent_path).unwrap() {
            fs::create_dir_all(dep_parent_path).unwrap();
        }
        if !fs::exists(&dep_path).unwrap() {
            fs::write(&dep_path, entry_runtime_code).unwrap();
        }

        dep_path.to_string_lossy().to_string()
    }

    fn get_entry_runtime_code(&self) -> String {
        let (plugins_imports, plugins_instantiations) = self.get_mf_runtime_plugins_code();

        format!(
            r#"import federation from "{federation_impl}";
{plugins_imports}

if(!{federation_global}.runtime) {{
  var preFederation = {federation_global};
  {federation_global} = {{}};
  for(var key in federation) {{
    {federation_global}[key] = federation[key];
  }}
  for(var key in preFederation) {{
    {federation_global}[key] = preFederation[key];
  }}
}}

if(!{federation_global}.instance) {{
  {plugins_instantiations}
  {federation_global}.instance = {federation_global}.runtime.init({federation_global}.initOptions);
  if({federation_global}.attachShareScopeMap) {{
    {federation_global}.attachShareScopeMap({mako_require});
  }}
  if({federation_global}.installInitialConsumes) {{
    {federation_global}.installInitialConsumes();
  }}
}}
"#,
            plugins_imports = plugins_imports,
            plugins_instantiations = plugins_instantiations,
            federation_impl = self.config.implementation,
            federation_global = FEDERATION_GLOBAL,
            mako_require = MAKO_REQUIRE
        )
    }

    fn get_mf_runtime_plugins_code(&self) -> (String, String) {
        let (imported_plugin_names, import_plugin_instantiations) =
            self.config.runtime_plugins.iter().enumerate().fold(
                (Vec::new(), Vec::new()),
                |(mut names, mut stmts), (index, plugin)| {
                    names.push(format!("plugin_{}", index));
                    stmts.push(format!(r#"import plugin_{} from "{}";"#, index, plugin));
                    (names, stmts)
                },
            );

        let plugins_imports = import_plugin_instantiations.join("\n");

        let plugins_instantiations = if imported_plugin_names.is_empty() {
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
        (plugins_imports, plugins_instantiations)
    }

    fn get_container_entry_code(&self, root: &Path) -> String {
        let exposes_modules_code = self
            .config
            .exposes
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, module)| {
                format!(
                    r#""{name}": () => import("{module}"),"#,
                    module = root.join(module).to_string_lossy()
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

        format!(
            r#"var moduleMap = {{
 {exposes_modules_code}
}};

var get = (module, getScope) => {{
    {mako_require}.R = getScope;
	getScope = (
	Object.prototype.hasOwnProperty.call(moduleMap, module)
			? moduleMap[module]()
			: Promise.resolve().then(() => {{
				throw new Error('Module "' + module + '" does not exist in container.');
			}})
	);
	{mako_require}.R = undefined;
	return getScope;
}};

var init = (shareScope, initScope, remoteEntryInitOptions) => {{
	return {mako_require}.federation.bundlerRuntime.initContainerEntry({{
        webpackRequire: {mako_require},
		shareScope: shareScope,
		initScope: initScope,
		remoteEntryInitOptions: remoteEntryInitOptions,
		shareScopeKey: "{share_scope}"
	}})
}};

export {{ get, init }};
"#,
            exposes_modules_code = exposes_modules_code,
            mako_require = MAKO_REQUIRE,
            share_scope = self.config.share_scope
        )
    }
}
