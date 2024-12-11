use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::fs;
use std::path::Path;

use serde::Serialize;
use tracing::warn;

use super::constants::{
    FEDERATION_EXPOSE_CHUNK_PREFIX, FEDERATION_GLOBAL, FEDERATION_REMOTE_REFERENCE_PREFIX,
};
use super::util::parse_remote;
use super::ModuleFederationPlugin;
use crate::config::{
    Config, ExternalAdvanced, ExternalAdvancedSubpath, ExternalAdvancedSubpathRule,
    ExternalAdvancedSubpathTarget, ExternalConfig,
};
use crate::module::md5_hash;
use crate::visitors::mako_require::MAKO_REQUIRE;

impl ModuleFederationPlugin {
    pub(super) fn prepare_entry_runtime_dep(&self, root: &Path) -> String {
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

    pub(super) fn get_entry_runtime_code(&self) -> String {
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

    pub(super) fn add_container_entry(&self, config: &mut Config, root: &Path) {
        // add container entry
        if let Some(exposes) = self.config.exposes.as_ref() {
            let container_entry_name = &self.config.name;
            if !exposes.is_empty() {
                match config.entry.entry(container_entry_name.clone()) {
                    Occupied(_) => {
                        warn!(
                            "mf exposed name {} is conflicting with entry config.",
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
                            fs::create_dir_all(container_entry_parent_path).unwrap();
                        }
                        fs::write(&container_entry_path, container_entry_code).unwrap();

                        vacant_entry.insert(container_entry_path);
                    }
                }
            }
        }
    }

    pub(super) fn get_container_entry_code(&self, root: &Path) -> String {
        let exposes_modules_code = self
            .config
            .exposes
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, module)| {
                format!(
                    r#""{name}": () => import(
                        /* makoChunkName: "{prefix}{striped_name}" */
                        /* federationExpose: true */
                        "{module}"
),"#,
                    prefix = FEDERATION_EXPOSE_CHUNK_PREFIX,
                    module = root.join(module).canonicalize().unwrap().to_string_lossy(),
                    striped_name = name.replace("./", "")
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

    pub(super) fn get_federation_runtime_code(&self) -> String {
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

    pub(super) fn get_federation_exposes_library_code(&self) -> String {
        if let Some(exposes) = self.config.exposes.as_ref() {
            if !exposes.is_empty() {
                format!(
                    r#"global["{}"] = requireModule(entryModuleId);
"#,
                    self.config.name
                )
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        }
    }

    #[allow(dead_code)]
    pub(super) fn append_remotes_externals(&self, config: &mut Config) {
        if let Some(remotes) = &self.config.remotes {
            remotes.iter().for_each(|remote| {
                config.externals.insert(
                    format!("{}{}", FEDERATION_REMOTE_REFERENCE_PREFIX, remote.0),
                    ExternalConfig::Advanced(ExternalAdvanced {
                        root: remote.0.clone(),
                        script: parse_remote(remote.1).ok().map(|(_, url)| url.clone()),
                        module_type: None,
                        subpath: Some(ExternalAdvancedSubpath {
                            exclude: None,
                            rules: vec![ExternalAdvancedSubpathRule {
                                regex: "/.*".to_string(),
                                target: ExternalAdvancedSubpathTarget::Empty,
                                target_converter: None,
                            }],
                        }),
                    }),
                );
            });
        }
    }

    pub(super) fn get_mf_runtime_plugins_code(&self) -> (String, String) {
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRemoteItem {
    name: String,
    alias: String,
    entry: String,
    share_scope: String,
}

#[derive(Serialize)]
struct RuntimeInitOptions {
    name: String,
    remotes: Vec<RuntimeRemoteItem>,
    share_strategy: String,
}
