use anyhow::{bail, Context, Result};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use turbo_rcstr::RcStr;
use turbo_tasks::{trace::TraceRawVcs, FxIndexMap, NonLocalValue, OperationValue, ResolvedVc, Vc};
use turbo_tasks_env::EnvMap;
use turbopack::module_options::{LoaderRuleItem, OptionWebpackRules};
use turbopack_core::resolve::ResolveAliasMap;
use turbopack_ecmascript::{OptionTreeShaking, TreeShakingMode};
use turbopack_ecmascript_plugins::transform::{
    emotion::EmotionTransformConfig, styled_components::StyledComponentsTransformConfig,
};
use turbopack_node::transforms::webpack::{WebpackLoaderItem, WebpackLoaderItems};

use super::{
    experimental::{EsmExternals, ExperimentalConfig},
    mode::Mode,
};
use crate::{
    config::turbo::{
        LoaderItem, ModuleIdStrategy, RuleConfigItem, RuleConfigItemOptions,
        RuleConfigItemOrShortcut,
    },
    transforms::modularize_imports::ModularizeImportPackageConfig,
};

#[turbo_tasks::value(serialization = "custom", eq = "manual")]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, OperationValue)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub env: FxIndexMap<String, JsonValue>,
    pub experimental: ExperimentalConfig,
    pub transpile_packages: Option<Vec<RcStr>>,
    pub modularize_imports: Option<FxIndexMap<String, ModularizeImportPackageConfig>>,
    pub dist_dir: Option<RcStr>,
    pub asset_prefix: Option<RcStr>,
    pub base_path: Option<RcStr>,
    pub cross_origin: Option<CrossOriginConfig>,
    pub output: Option<OutputType>,
    pub compiler: Option<CompilerConfig>,
    pub cache_handler: Option<RcStr>,
}

#[turbo_tasks::value_impl]
impl Config {
    #[turbo_tasks::function]
    pub async fn from_string(string: Vc<RcStr>) -> Result<Vc<Self>> {
        let string = string.await?;
        let config: Config = serde_json::from_str(&string)
            .with_context(|| format!("failed to parse next.config.js: {}", string))?;
        Ok(config.cell())
    }

    #[turbo_tasks::function]
    pub fn is_standalone(&self) -> Vc<bool> {
        Vc::cell(self.output == Some(OutputType::Standalone))
    }

    #[turbo_tasks::function]
    pub fn cache_handler(&self) -> Vc<Option<RcStr>> {
        Vc::cell(self.cache_handler.clone())
    }

    #[turbo_tasks::function]
    pub fn compiler(&self) -> Vc<CompilerConfig> {
        self.compiler.clone().unwrap_or_default().cell()
    }

    #[turbo_tasks::function]
    pub fn env(&self) -> Vc<EnvMap> {
        // The value expected for env is Record<String, String>, but config itself
        // allows arbitrary object (https://github.com/vercel/next.js/blob/25ba8a74b7544dfb6b30d1b67c47b9cb5360cb4e/packages/next/src/server/config-schema.ts#L203)
        // then stringifies it. We do the interop here as well.
        let env = self
            .env
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().into(),
                    if let JsonValue::String(s) = v {
                        // A string value is kept, calling `to_string` would wrap in to quotes.
                        s.as_str().into()
                    } else {
                        v.to_string().into()
                    },
                )
            })
            .collect();

        Vc::cell(env)
    }

    #[turbo_tasks::function]
    pub fn transpile_packages(&self) -> Vc<Vec<RcStr>> {
        Vc::cell(self.transpile_packages.clone().unwrap_or_default())
    }

    #[turbo_tasks::function]
    pub fn webpack_rules(&self, active_conditions: Vec<RcStr>) -> Vc<OptionWebpackRules> {
        let Some(turbo_rules) = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|t| t.rules.as_ref())
        else {
            return Vc::cell(None);
        };
        if turbo_rules.is_empty() {
            return Vc::cell(None);
        }
        let active_conditions = active_conditions.into_iter().collect::<FxHashSet<_>>();
        let mut rules = FxIndexMap::default();
        for (ext, rule) in turbo_rules.iter() {
            fn transform_loaders(loaders: &[LoaderItem]) -> ResolvedVc<WebpackLoaderItems> {
                ResolvedVc::cell(
                    loaders
                        .iter()
                        .map(|item| match item {
                            LoaderItem::LoaderName(name) => WebpackLoaderItem {
                                loader: name.clone(),
                                options: Default::default(),
                            },
                            LoaderItem::LoaderOptions(options) => options.clone(),
                        })
                        .collect(),
                )
            }
            enum FindRuleResult<'a> {
                Found(&'a RuleConfigItemOptions),
                NotFound,
                Break,
            }
            fn find_rule<'a>(
                rule: &'a RuleConfigItem,
                active_conditions: &FxHashSet<RcStr>,
            ) -> FindRuleResult<'a> {
                match rule {
                    RuleConfigItem::Options(rule) => FindRuleResult::Found(rule),
                    RuleConfigItem::Conditional(map) => {
                        for (condition, rule) in map.iter() {
                            if condition == "default" || active_conditions.contains(condition) {
                                match find_rule(rule, active_conditions) {
                                    FindRuleResult::Found(rule) => {
                                        return FindRuleResult::Found(rule);
                                    }
                                    FindRuleResult::Break => {
                                        return FindRuleResult::Break;
                                    }
                                    FindRuleResult::NotFound => {}
                                }
                            }
                        }
                        FindRuleResult::NotFound
                    }
                    RuleConfigItem::Boolean(_) => FindRuleResult::Break,
                }
            }
            match rule {
                RuleConfigItemOrShortcut::Loaders(loaders) => {
                    rules.insert(
                        ext.clone(),
                        LoaderRuleItem {
                            loaders: transform_loaders(loaders),
                            rename_as: None,
                        },
                    );
                }
                RuleConfigItemOrShortcut::Advanced(rule) => {
                    if let FindRuleResult::Found(RuleConfigItemOptions { loaders, rename_as }) =
                        find_rule(rule, &active_conditions)
                    {
                        rules.insert(
                            ext.clone(),
                            LoaderRuleItem {
                                loaders: transform_loaders(loaders),
                                rename_as: rename_as.clone(),
                            },
                        );
                    }
                }
            }
        }
        Vc::cell(Some(ResolvedVc::cell(rules)))
    }

    #[turbo_tasks::function]
    pub fn persistent_caching_enabled(&self) -> Result<Vc<bool>> {
        Ok(Vc::cell(
            self.experimental
                .turbo
                .as_ref()
                .and_then(|t| t.unstable_persistent_caching)
                .unwrap_or_default(),
        ))
    }

    #[turbo_tasks::function]
    pub fn resolve_alias_options(&self) -> Result<Vc<ResolveAliasMap>> {
        let Some(resolve_alias) = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|t| t.resolve_alias.as_ref())
        else {
            return Ok(ResolveAliasMap::cell(ResolveAliasMap::default()));
        };
        let alias_map: ResolveAliasMap = resolve_alias.try_into()?;
        Ok(alias_map.cell())
    }

    #[turbo_tasks::function]
    pub fn resolve_extension(&self) -> Vc<ResolveExtensions> {
        let Some(resolve_extensions) = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|t| t.resolve_extensions.as_ref())
        else {
            return Vc::cell(None);
        };
        Vc::cell(Some(resolve_extensions.clone()))
    }

    #[turbo_tasks::function]
    pub async fn import_externals(&self) -> Result<Vc<bool>> {
        Ok(Vc::cell(match self.experimental.esm_externals {
            Some(EsmExternals::Bool(b)) => b,
            Some(EsmExternals::Loose(_)) => bail!("esmExternals = \"loose\" is not supported"),
            None => true,
        }))
    }

    #[turbo_tasks::function]
    pub fn modularize_imports(&self) -> Vc<ModularizeImports> {
        Vc::cell(self.modularize_imports.clone().unwrap_or_default())
    }

    #[turbo_tasks::function]
    pub fn experimental_swc_plugins(&self) -> Vc<SwcPlugins> {
        Vc::cell(self.experimental.swc_plugins.clone().unwrap_or_default())
    }

    /// Returns the final asset prefix. If an assetPrefix is set, it's used.
    /// Otherwise, the basePath is used.
    #[turbo_tasks::function]
    pub async fn computed_asset_prefix(self: Vc<Self>) -> Result<Vc<Option<RcStr>>> {
        let this = self.await?;

        Ok(Vc::cell(Some(
            format!(
                "{}/_next/",
                if let Some(asset_prefix) = &this.asset_prefix {
                    asset_prefix
                } else {
                    this.base_path.as_ref().map_or("", |b| b.as_str())
                }
                .trim_end_matches('/')
            )
            .into(),
        )))
    }

    #[turbo_tasks::function]
    pub fn enable_taint(&self) -> Vc<bool> {
        Vc::cell(self.experimental.taint.unwrap_or(false))
    }

    #[turbo_tasks::function]
    pub fn enable_react_owner_stack(&self) -> Vc<bool> {
        Vc::cell(self.experimental.react_owner_stack.unwrap_or(false))
    }

    #[turbo_tasks::function]
    pub fn enable_view_transition(&self) -> Vc<bool> {
        Vc::cell(self.experimental.view_transition.unwrap_or(false))
    }

    #[turbo_tasks::function]
    pub fn enable_dynamic_io(&self) -> Vc<bool> {
        Vc::cell(self.experimental.dynamic_io.unwrap_or(false))
    }

    #[turbo_tasks::function]
    pub fn enable_use_cache(&self) -> Vc<bool> {
        Vc::cell(
            self.experimental
                .use_cache
                // "use cache" was originally implicitly enabled with the
                // dynamicIO flag, so we transfer the value for dynamicIO to the
                // explicit useCache flag to ensure backwards compatibility.
                .unwrap_or(self.experimental.dynamic_io.unwrap_or(false)),
        )
    }

    #[turbo_tasks::function]
    pub fn cache_kinds(&self) -> Vc<CacheKinds> {
        let mut cache_kinds = CacheKinds::default();

        if let Some(handlers) = self.experimental.cache_handlers.as_ref() {
            cache_kinds.extend(handlers.keys().cloned());
        }

        cache_kinds.cell()
    }

    #[turbo_tasks::function]
    pub fn optimize_package_imports(&self) -> Vc<Vec<RcStr>> {
        Vc::cell(
            self.experimental
                .optimize_package_imports
                .clone()
                .unwrap_or_default(),
        )
    }

    #[turbo_tasks::function]
    pub fn tree_shaking_mode_for_foreign_code(
        &self,
        _is_development: bool,
    ) -> Vc<OptionTreeShaking> {
        let tree_shaking = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|v| v.tree_shaking);

        OptionTreeShaking(match tree_shaking {
            Some(false) => Some(TreeShakingMode::ReexportsOnly),
            Some(true) => Some(TreeShakingMode::ModuleFragments),
            None => Some(TreeShakingMode::ReexportsOnly),
        })
        .cell()
    }

    #[turbo_tasks::function]
    pub fn tree_shaking_mode_for_user_code(&self, _is_development: bool) -> Vc<OptionTreeShaking> {
        let tree_shaking = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|v| v.tree_shaking);

        OptionTreeShaking(match tree_shaking {
            Some(false) => Some(TreeShakingMode::ReexportsOnly),
            Some(true) => Some(TreeShakingMode::ModuleFragments),
            None => Some(TreeShakingMode::ReexportsOnly),
        })
        .cell()
    }

    #[turbo_tasks::function]
    pub fn module_id_strategy_config(&self) -> Vc<OptionModuleIdStrategy> {
        let Some(module_id_strategy) = self
            .experimental
            .turbo
            .as_ref()
            .and_then(|t| t.module_id_strategy)
        else {
            return Vc::cell(None);
        };
        Vc::cell(Some(module_id_strategy))
    }

    #[turbo_tasks::function]
    pub async fn turbo_minify(&self, mode: Vc<Mode>) -> Result<Vc<bool>> {
        let minify = self.experimental.turbo.as_ref().and_then(|t| t.minify);

        Ok(Vc::cell(
            minify.unwrap_or(matches!(*mode.await?, Mode::Build)),
        ))
    }

    #[turbo_tasks::function]
    pub async fn turbo_source_maps(&self) -> Result<Vc<bool>> {
        let source_maps = self.experimental.turbo.as_ref().and_then(|t| t.source_maps);

        Ok(Vc::cell(source_maps.unwrap_or(true)))
    }
}

#[turbo_tasks::value(transparent)]
pub struct OptionModuleIdStrategy(pub Option<ModuleIdStrategy>);

#[turbo_tasks::value(transparent)]
#[derive(Clone, Debug)]
pub struct CacheKinds(FxHashSet<RcStr>);

impl CacheKinds {
    pub fn extend<I: IntoIterator<Item = RcStr>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

impl Default for CacheKinds {
    fn default() -> Self {
        CacheKinds(["default", "remote"].iter().map(|&s| s.into()).collect())
    }
}

#[turbo_tasks::value(transparent)]
pub struct ModularizeImports(FxIndexMap<String, ModularizeImportPackageConfig>);

#[turbo_tasks::value(transparent)]
pub struct ResolveExtensions(Option<Vec<RcStr>>);

#[turbo_tasks::value(transparent)]
pub struct SwcPlugins(Vec<(RcStr, serde_json::Value)>);

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "kebab-case")]
pub enum OutputType {
    Standalone,
    Export,
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "kebab-case")]
pub enum CrossOriginConfig {
    Anonymous,
    UseCredentials,
}

#[turbo_tasks::value(eq = "manual")]
#[derive(Clone, Debug, PartialEq, Default, OperationValue)]
#[serde(rename_all = "camelCase")]
pub struct CompilerConfig {
    pub emotion: Option<EmotionTransformOptionsOrBoolean>,
    pub remove_console: Option<RemoveConsoleConfig>,
    pub styled_components: Option<StyledComponentsTransformOptionsOrBoolean>,
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum RemoveConsoleConfig {
    Boolean(bool),
    Config { exclude: Option<Vec<String>> },
}

impl RemoveConsoleConfig {
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Boolean(enabled) => *enabled,
            _ => true,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum StyledComponentsTransformOptionsOrBoolean {
    Boolean(bool),
    Options(StyledComponentsTransformConfig),
}

impl StyledComponentsTransformOptionsOrBoolean {
    pub fn is_enabled(&self) -> bool {
        match self {
            Self::Boolean(enabled) => *enabled,
            _ => true,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum EmotionTransformOptionsOrBoolean {
    Boolean(bool),
    Options(EmotionTransformConfig),
}
