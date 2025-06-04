use anyhow::Result;
use turbo_tasks::{ResolvedVc, Value, Vc};
use turbo_tasks_fs::{self, glob::Glob, FileSystemPath};
use turbopack_core::{
    reference_type::ReferenceType,
    resolve::{
        parse::Request,
        pattern::Pattern,
        plugin::{BeforeResolvePlugin, BeforeResolvePluginCondition},
        ExternalTraced, ExternalType, ResolveResult, ResolveResultItem, ResolveResultOption,
    },
};

use crate::config::{ExternalConfig, ExternalsConfig};

#[turbo_tasks::value]
pub struct ExternalsPlugin {
    project_path: ResolvedVc<FileSystemPath>,
    root: ResolvedVc<FileSystemPath>,
    externals_config: ResolvedVc<ExternalsConfig>,
}

#[turbo_tasks::value_impl]
impl ExternalsPlugin {
    #[turbo_tasks::function]
    pub fn new(
        project_path: ResolvedVc<FileSystemPath>,
        root: ResolvedVc<FileSystemPath>,
        externals_config: ResolvedVc<ExternalsConfig>,
    ) -> Vc<Self> {
        ExternalsPlugin {
            project_path,
            root,
            externals_config,
        }
        .cell()
    }
}

#[turbo_tasks::value_impl]
impl BeforeResolvePlugin for ExternalsPlugin {
    #[turbo_tasks::function]
    fn before_resolve_condition(&self) -> Vc<BeforeResolvePluginCondition> {
        BeforeResolvePluginCondition::from_request_glob(Glob::new("*".into()))
    }

    #[turbo_tasks::function]
    async fn before_resolve(
        &self,
        _lookup_path: ResolvedVc<FileSystemPath>,
        _reference_type: Value<ReferenceType>,
        request: Vc<Request>,
    ) -> Result<Vc<ResolveResultOption>> {
        let externals_config = self.externals_config.await?;
        let request_value = request.await?;

        // get request module name
        let module_name = match &*request_value {
            Request::Module { module, .. } => module,
            Request::Raw {
                path: Pattern::Constant(name),
                ..
            } => name,
            _ => return Ok(ResolveResultOption::none()),
        };

        // check if the module exists in externals config.
        if let Some(external_config) = externals_config.get(module_name) {
            let (external_name, external_type) = match external_config {
                ExternalConfig::Basic(name) => {
                    // resolve basic config like "foo" or "commonjs foo" or "esm foo" or "script https://..."
                    let name_str = name.as_str();
                    if name_str.starts_with("commonjs ") {
                        let actual_name = name_str.strip_prefix("commonjs ").unwrap_or(name_str);
                        (actual_name.into(), ExternalType::CommonJs)
                    } else if name_str.starts_with("esm ") {
                        let actual_name = name_str.strip_prefix("esm ").unwrap_or(name_str);
                        (actual_name.into(), ExternalType::EcmaScriptModule)
                    } else if name_str.starts_with("script ") {
                        let script_content = name_str.strip_prefix("script ").unwrap_or(name_str);
                        // For script type in basic config, check if script_content already contains '@' separator
                        let external_name = if script_content.contains('@') {
                            // If already in root@script format, use it directly
                            script_content.to_string()
                        } else {
                            // Otherwise, concatenate module_name and script URL with '@' separator
                            // Format: module_name@script_url
                            format!("{}@{}", module_name, script_content)
                        };
                        (external_name.into(), ExternalType::Script)
                    } else {
                        // Default to Global
                        (name.clone(), ExternalType::Global)
                    }
                }
                ExternalConfig::Advanced(advanced) => {
                    // advanced config.
                    let external_type = match &advanced.r#type {
                        Some(crate::config::ExternalType::CommonJs) => ExternalType::CommonJs,
                        Some(crate::config::ExternalType::ESM) => ExternalType::EcmaScriptModule,
                        Some(crate::config::ExternalType::Script) => {
                            // For script type, concatenate root and script URL with '@' separator
                            // Format: root@script
                            let external_name = if let Some(script_url) = &advanced.script {
                                format!("{}@{}", advanced.root, script_url)
                            } else {
                                // If no script URL is provided, just use root
                                advanced.root.to_string()
                            };
                            return Ok(ResolveResultOption::some(*ResolveResult::primary(
                                ResolveResultItem::External {
                                    name: external_name.into(),
                                    ty: ExternalType::Script,
                                    traced: ExternalTraced::Traced,
                                },
                            )));
                        }
                        Some(crate::config::ExternalType::Global) => ExternalType::Global,
                        None => ExternalType::Global,
                    };
                    (advanced.root.clone(), external_type)
                }
            };

            return Ok(ResolveResultOption::some(*ResolveResult::primary(
                ResolveResultItem::External {
                    name: external_name,
                    ty: external_type,
                    traced: ExternalTraced::Traced,
                },
            )));
        }

        Ok(ResolveResultOption::none())
    }
}
