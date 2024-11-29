use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::Serialize;
use tracing::warn;

use crate::ast::file::{Content, JsContent};
use crate::compiler::{Args, Context};
use crate::config::module_federation::ModuleFederationConfig;
use crate::config::{
    Config, ExternalAdvanced, ExternalAdvancedSubpath, ExternalAdvancedSubpathRule,
    ExternalAdvancedSubpathTarget, ExternalConfig,
};
use crate::module::md5_hash;
use crate::plugin::{Plugin, PluginResolveIdParams};
use crate::resolve::{RemoteInfo, ResolverResource};
use crate::visitors::mako_require::MAKO_REQUIRE;

pub struct ModuleFederationPlugin {
    pub config: ModuleFederationConfig,
}

const FEDERATION_GLOBAL: &str = "__mako_require__.federation";

const FEDERATION_REMOTE_MODULE_PREFIX: &str = "mako/container/remote/";

const FEDERATION_REMOTE_REFERENCE_PREFIX: &str = "mako/container/reference/";

const FEDERATION_SHARED_REFERENCE_PREFIX: &str = "mako/sharing/consume/";

impl ModuleFederationPlugin {
    pub fn new(config: ModuleFederationConfig) -> Self {
        Self { config }
    }

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
    fn get_container_references_code(&self, context: &Arc<Context>) -> String {
        let module_graph = context.module_graph.read().unwrap();
        let chunk_graph = context.chunk_graph.read().unwrap();
        let all_chunks = chunk_graph.get_all_chunks();

        let mut chunk_mapping: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut id_to_external_and_name_mapping: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut id_to_remote_map: HashMap<&str, Vec<RemoteExternal>> = HashMap::new();
        all_chunks.iter().for_each(|c| {
            c.modules.iter().for_each(|m| {
                if let Some(m) = module_graph.get_module(m) {
                    if m.is_remote {
                        {
                            chunk_mapping
                                .entry(c.id.id.as_str())
                                .or_default()
                                .push(m.id.id.as_str());
                        }

                        {
                            let remote_module = m
                                .info
                                .as_ref()
                                .unwrap()
                                .resolved_resource
                                .as_ref()
                                .unwrap()
                                .get_remote_info()
                                .unwrap();
                            let remote_info = id_to_external_and_name_mapping
                                .entry(m.id.id.as_str())
                                .or_default();
                            remote_info.push(&remote_module.share_scope);
                            remote_info.push(&remote_module.sub_path);
                            remote_info.push(&remote_module.external_refenrence_id);

                            let external_info =
                                id_to_remote_map.entry(m.id.id.as_str()).or_default();

                            external_info.push(RemoteExternal {
                                name: remote_module.name.clone(),
                                external_type: remote_module.external_type.clone(),
                                external_module_id: remote_module.external_refenrence_id.clone(),
                            });
                        }
                    }
                }
            });
        });

        let chunk_mapping = serde_json::to_string(&chunk_mapping).unwrap();
        let id_to_external_and_name_mapping =
            serde_json::to_string(&id_to_external_and_name_mapping).unwrap();
        let id_to_remote_map = serde_json::to_string(&id_to_remote_map).unwrap();

        format!(
            r#"
/* mako/runtime/federation remotes consume */
!(function() {{
  var chunkMapping = {chunk_mapping};
  var idToExternalAndNameMapping = {id_to_external_and_name_mapping};
  var idToRemoteMap = {id_to_remote_map};
  requireModule.federation.bundlerRuntimeOptions.remotes = {{idToRemoteMap, chunkMapping, idToExternalAndNameMapping, webpackRequire: requireModule}};
  requireModule.chunkEnsures.remotes = (chunkId, promises) => {{
    requireModule.federation.bundlerRuntime.remotes({{ idToRemoteMap,chunkMapping, idToExternalAndNameMapping, chunkId, promises, webpackRequire: requireModule}});
  }}
}}
)()"#,
        )
    }

    fn add_container_entry(&self, config: &mut Config, root: &Path) {
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
    }

    fn append_remotes_externals(&self, config: &mut Config) {
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
}

impl Plugin for ModuleFederationPlugin {
    fn name(&self) -> &str {
        "module_federation"
    }

    fn modify_config(&self, config: &mut Config, root: &Path, _args: &Args) -> Result<()> {
        self.add_container_entry(config, root);

        self.append_remotes_externals(config);

        Ok(())
    }

    fn load(
        &self,
        _param: &crate::plugin::PluginLoadParam,
        _context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        Ok(_param.file.path().map_or_else(
            || None,
            |path| {
                if path.starts_with(FEDERATION_REMOTE_MODULE_PREFIX) {
                    Some(Content::Js(JsContent {
                        is_jsx: false,
                        content: "".to_string(),
                    }))
                } else {
                    None
                }
            },
        ))
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

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let federation_runtime_code = self.get_federation_runtime_code();
        let federation_container_references_code = self.get_container_references_code(context);

        Ok(vec![
            federation_runtime_code,
            federation_container_references_code,
        ])
    }

    fn resolve_id(
        &self,
        source: &str,
        _importer: &str,
        _params: &PluginResolveIdParams,
        _context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        let source_parts = source
            .split_once("/")
            .map_or((source.to_string(), ".".to_string()), |(part_0, part_1)| {
                (part_0.to_string(), part_1.to_string())
            });
        Ok(self.config.remotes.as_ref().map_or_else(
            || None,
            |remotes| {
                remotes.get(&source_parts.0).map(|_remote| {
                    ResolverResource::Remote(RemoteInfo {
                        module_id: format!("{}{}", FEDERATION_REMOTE_MODULE_PREFIX, source),
                        external_refenrence_id: format!(
                            "{}{}",
                            FEDERATION_REMOTE_REFERENCE_PREFIX, source_parts.0
                        ),
                        // FIXME: hard code now
                        external_type: "script".to_string(),
                        sub_path: format!("./{}", source_parts.1),
                        name: source_parts.0.to_string(),
                        share_scope: self.config.share_scope.clone(),
                    })
                })
            },
        ))
    }
}

#[derive(Serialize)]
struct RuntimeInitOptions {
    name: String,
    remotes: Vec<RuntimeRemoteItem>,
    share_strategy: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRemoteItem {
    name: String,
    alias: String,
    entry: String,
    share_scope: String,
}

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteExternal {
    external_type: String,
    name: String,
    external_module_id: String,
}
