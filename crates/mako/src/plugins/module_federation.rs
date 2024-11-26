use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::Serialize;
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
        // add containter entry
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
        // add containter entry runtime dependency
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
        let federation_runtime_code = self.get_federation_runtime_code();
        let federation_container_references_code = self.get_container_references_code();

        Ok(vec![
            federation_runtime_code,
            federation_container_references_code,
        ])
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
                    names.push(format!("plugin_{index}"));
                    stmts.push(format!(r#"import plugin_{index} from "{plugin}";"#));
                    (names, stmts)
                },
            );

        let plugins_imports = import_plugin_instantiations.join("\n");

        let plugins_to_add = imported_plugin_names
            .iter()
            .map(|item| format!(r#"{item} ? (item.default || item)() : false"#))
            .collect::<Vec<_>>()
            .join(",");

        let plugins_instantiations = if imported_plugin_names.is_empty() {
            "".to_string()
        } else {
            format!(
                r#"var pluginsToAdd = [{plugins_to_add}].filter(Boolean);
  {federation_global}.initOptions.plugins = {federation_global}.initOptions.plugins ?
    {federation_global}.initOptions.plugins.concat(pluginsToAdd) : pluginsToAdd;
"#,
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
                    r#""{name}": () => import(/* makoChunkName: "__mf_expose_{container_name}" */ "{module}"),"#,
                    container_name = self.config.name,
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
            mako_require = MAKO_REQUIRE,
            share_scope = self.config.share_scope
        )
    }

    fn get_federation_runtime_code(&self) -> String {
        fn parse_remote(remote: &str) -> Result<(String, String)> {
            let (left, right) = remote
                .split_once('@')
                .ok_or(anyhow!("invalid remote {}", remote))?;
            if left.is_empty() || right.is_empty() {
                Err(anyhow!("invalid remote {}", remote))
            } else {
                Ok((left.to_string(), right.to_string()))
            }
        }

        let runtime_remotes = self.config.remotes.as_ref().map_or(Vec::new(), |remotes| {
            remotes
                .iter()
                .map(|(alias, remote)| {
                    // FIXME: should not unwrap
                    let (name, entry) = parse_remote(remote).unwrap();
                    RuntimeRemoteItem {
                        name,
                        alias: alias.clone(),
                        entry,
                        share_scope: self.config.share_scope.clone(),
                    }
                })
                .collect()
        });
        let init_options: RuntimeInitOptions = RuntimeInitOptions {
            name: self.config.name.clone(),
            remotes: runtime_remotes,
            share_strategy: serde_json::to_value(&self.config.share_strategy)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
        };
        let init_options_code = serde_json::to_string(&init_options).unwrap();

        let federation_runtime_code = format!(
            r#"
/* mako/runtime/federation runtime */
!(function() {{
  if(!requireModule.federation) {{
    requireModule.federation = {{
      initOptions: {init_options_code},
      chunkMatcher: () => true,
      rootOutputDir: "",
      initialConsumes: undefined,
      bundlerRuntimeOptions: {{}}
    }};
  }}
}})();"#
        );
        federation_runtime_code
    }

    // TODO: impl remote module
    fn get_container_references_code(&self) -> String {
        "".to_string()
    }
}

#[derive(Serialize)]
struct RuntimeInitOptions {
    name: String,
    remotes: Vec<RuntimeRemoteItem>,
    share_strategy: String,
}

#[derive(Serialize)]
struct RuntimeRemoteItem {
    name: String,
    alias: String,
    entry: String,
    share_scope: String,
}
