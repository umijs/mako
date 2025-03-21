use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use turbo_rcstr::RcStr;
use turbo_tasks::{trace::TraceRawVcs, FxIndexMap, NonLocalValue, OperationValue};
use turbopack_node::transforms::webpack::WebpackLoaderItem;

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Deserialize,
    Serialize,
    TraceRawVcs,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalTurboConfig {
    /// This option has been replaced by `rules`.
    pub loaders: Option<JsonValue>,
    pub rules: Option<FxIndexMap<RcStr, RuleConfigItemOrShortcut>>,
    pub resolve_alias: Option<FxIndexMap<RcStr, JsonValue>>,
    pub resolve_extensions: Option<Vec<RcStr>>,
    pub tree_shaking: Option<bool>,
    pub module_id_strategy: Option<ModuleIdStrategy>,
    pub minify: Option<bool>,
    pub source_maps: Option<bool>,
    pub unstable_persistent_caching: Option<bool>,
}

#[turbo_tasks::value(operation)]
#[derive(Copy, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ModuleIdStrategy {
    Named,
    Deterministic,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RuleConfigItemOrShortcut {
    Loaders(Vec<LoaderItem>),
    Advanced(RuleConfigItem),
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum LoaderItem {
    LoaderName(RcStr),
    LoaderOptions(WebpackLoaderItem),
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RuleConfigItem {
    Options(RuleConfigItemOptions),
    Conditional(FxIndexMap<RcStr, RuleConfigItem>),
    Boolean(bool),
}

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct RuleConfigItemOptions {
    pub loaders: Vec<LoaderItem>,
    #[serde(default, alias = "as")]
    pub rename_as: Option<RcStr>,
}
