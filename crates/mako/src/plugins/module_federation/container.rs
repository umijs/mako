use std::fs;
use std::path::Path;

use serde::Serialize;
use tracing::warn;

use super::constants::{FEDERATION_EXPOSE_CHUNK_PREFIX, FEDERATION_GLOBAL};
use super::util::parse_remote;
use super::ModuleFederationPlugin;
use crate::config::{AllowChunks, Config};
use crate::module::md5_hash;
use crate::visitors::mako_require::MAKO_REQUIRE;

impl ModuleFederationPlugin {
    pub(super) fn add_container_entry(&self, config: &mut Config, root: &Path) {
        // add container entry
        if let Some(exposes) = self.config.exposes.as_ref() {
            let container_entry_name = &self.config.name;
            if !exposes.is_empty() {
                match config.entry.entry(container_entry_name.clone()) {
                    indexmap::map::Entry::Occupied(_) => {
                        warn!(
                            "mf exposed name {} is conflicting with entry config.",
                            container_entry_name
                        );
                    }
                    indexmap::map::Entry::Vacant(vacant_entry) => {
                        // TODO: refactor with virtual entry
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
                        /* makoChunkName: "{FEDERATION_EXPOSE_CHUNK_PREFIX}{striped_name}" */
                        /* federationExpose: true */
                        "{module}"
),"#,
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
    {MAKO_REQUIRE}.R = getScope;
	getScope = (
	Object.prototype.hasOwnProperty.call(moduleMap, module)
			? moduleMap[module]()
			: Promise.resolve().then(() => {{
				throw new Error('Module "' + module + '" does not exist in container.');
			}})
	);
	{MAKO_REQUIRE}.R = undefined;
	return getScope;
}};

var init = (shareScope, initScope, remoteEntryInitOptions) => {{
	return {FEDERATION_GLOBAL}.bundlerRuntime.initContainerEntry({{
        webpackRequire: {MAKO_REQUIRE},
		shareScope: shareScope,
		initScope: initScope,
		remoteEntryInitOptions: remoteEntryInitOptions,
		shareScopeKey: "{share_scope}"
	}})
}};

export {{ get, init }};
"#,
            share_scope = self.config.share_scope
        )
    }

    pub(super) fn prepare_container_entry_dep(&self, root: &Path) -> String {
        let container_content = self.get_federation_init_code();

        let content_hash = md5_hash(&container_content, 32);

        let dep_path = root.join(format!(
            "node_modules/.federation/.entry.{}.js",
            content_hash
        ));
        let dep_parent_path = dep_path.parent().unwrap();
        if !fs::exists(dep_parent_path).unwrap() {
            fs::create_dir_all(dep_parent_path).unwrap();
        }
        if !fs::exists(&dep_path).unwrap() {
            fs::write(&dep_path, container_content).unwrap();
        }

        dep_path.to_string_lossy().to_string()
    }

    pub(super) fn get_federation_init_code(&self) -> String {
        let (plugins_imports, plugins_instantiations) = self.get_mf_runtime_plugins_content();

        format!(
            r#"import federation from "{federation_impl}";
{plugins_imports}

if(!{FEDERATION_GLOBAL}.runtime) {{
  var preFederation = {FEDERATION_GLOBAL};
  {FEDERATION_GLOBAL} = {{}};
  for(var key in federation) {{
    {FEDERATION_GLOBAL}[key] = federation[key];
  }}
  for(var key in preFederation) {{
    {FEDERATION_GLOBAL}[key] = preFederation[key];
  }}
}}

if(!{FEDERATION_GLOBAL}.instance) {{
  {plugins_instantiations}
  {FEDERATION_GLOBAL}.instance = {FEDERATION_GLOBAL}.runtime.init({FEDERATION_GLOBAL}.initOptions);
  if({FEDERATION_GLOBAL}.attachShareScopeMap) {{
    {FEDERATION_GLOBAL}.attachShareScopeMap({MAKO_REQUIRE});
  }}
  if({FEDERATION_GLOBAL}.installInitialConsumes) {{
    {FEDERATION_GLOBAL}.installInitialConsumes();
  }}
}}
"#,
            federation_impl = self.config.implementation,
        )
    }

    pub(super) fn init_federation_runtime_options(&self) -> String {
        let runtime_remotes = self.config.remotes.as_ref().map_or(Vec::new(), |remotes| {
            remotes
                .iter()
                .map(|(alias, remote)| {
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

    pub(super) fn get_mf_runtime_plugins_content(&self) -> (String, String) {
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
  {FEDERATION_GLOBAL}.initOptions.plugins = {FEDERATION_GLOBAL}.initOptions.plugins ?
    {FEDERATION_GLOBAL}.initOptions.plugins.concat(pluginsToAdd) : pluginsToAdd;
"#,
            )
        };

        (plugins_imports, plugins_instantiations)
    }

    pub(super) fn export_federation_container(&self) -> String {
        if let Some(exposes) = self.config.exposes.as_ref() {
            if !exposes.is_empty() {
                format!(
                    r#"global["{}"] = requireModule(entryModuleId);"#,
                    self.config.name
                )
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        }
    }
    pub(crate) fn patch_code_splitting(
        &self,
        optimize_chunk_options: &mut crate::config::CodeSplittingAdvancedOptions,
    ) {
        optimize_chunk_options.groups.iter_mut().for_each(|group| {
            group.allow_chunks = AllowChunks::Async;
        });
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
