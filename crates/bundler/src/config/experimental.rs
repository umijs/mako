use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{
    debug::ValueDebugFormat, trace::TraceRawVcs, FxIndexMap, NonLocalValue, OperationValue,
};
use turbopack::module_options::MdxTransformOptions;

use super::turbo::ExperimentalTurboConfig;

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    TraceRawVcs,
    ValueDebugFormat,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalConfig {
    pub swc_plugins: Option<Vec<(RcStr, serde_json::Value)>>,
    pub turbo: Option<ExperimentalTurboConfig>,
    pub esm_externals: Option<EsmExternals>,
    pub inline_css: Option<bool>,
    pub optimize_package_imports: Option<Vec<RcStr>>,
    #[serde(rename = "dynamicIO")]
    pub dynamic_io: Option<bool>,
    pub use_cache: Option<bool>,
    pub cache_handlers: Option<FxIndexMap<RcStr, RcStr>>,
    // For react
    pub view_transition: Option<bool>,
    pub taint: Option<bool>,
    pub react_owner_stack: Option<bool>,
}

#[derive(
    Clone, Debug, PartialEq, Deserialize, Serialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(rename_all = "kebab-case")]
pub enum EsmExternalsValue {
    Loose,
}

#[derive(
    Clone, Debug, PartialEq, Deserialize, Serialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum EsmExternals {
    Loose(EsmExternalsValue),
    Bool(bool),
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, TraceRawVcs, NonLocalValue, OperationValue,
)]
#[serde(untagged)]
pub enum MdxRsOptions {
    Boolean(bool),
    Option(MdxTransformOptions),
}
