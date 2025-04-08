use anyhow::Result;
use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{trace::TraceRawVcs, NonLocalValue, TaskInput, Vc};
use turbo_tasks_fs::FileSystem;
use turbopack::condition::ContextCondition;

use crate::config::Config;

#[derive(
    Default,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Debug,
    TraceRawVcs,
    Serialize,
    Deserialize,
    Hash,
    PartialOrd,
    Ord,
    TaskInput,
    NonLocalValue,
)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    #[default]
    NodeJs,
    #[serde(alias = "experimental-edge")]
    Edge,
}

impl Runtime {
    pub fn conditions(&self) -> &'static [&'static str] {
        match self {
            Runtime::NodeJs => &["node"],
            Runtime::Edge => &["edge-light"],
        }
    }
}

#[turbo_tasks::function]
pub async fn get_transpiled_packages(config: Vc<Config>) -> Result<Vc<Vec<RcStr>>> {
    let transpile_packages: Vec<RcStr> = config.transpile_packages().owned().await?;

    Ok(Vc::cell(transpile_packages))
}

pub async fn foreign_code_context_condition(config: Vc<Config>) -> Result<ContextCondition> {
    let transpiled_packages = get_transpiled_packages(config).await?;

    let result = ContextCondition::all(vec![
        ContextCondition::InDirectory("node_modules".to_string()),
        ContextCondition::not(ContextCondition::any(
            transpiled_packages
                .iter()
                .map(|package| ContextCondition::InDirectory(format!("node_modules/{package}")))
                .collect(),
        )),
    ]);
    Ok(result)
}

/// Determines if the module is an internal asset (i.e overlay, fallback) coming from the embedded
/// FS, don't apply user defined transforms.
//
// TODO: Turbopack specific embed fs paths should be handled by internals of Turbopack itself and
// user config should not try to leak this. However, currently we apply few transform options
// subject to Next.js's configuration even if it's embedded assets.
pub async fn internal_assets_conditions() -> Result<ContextCondition> {
    Ok(ContextCondition::any(vec![
        ContextCondition::InPath(
            turbopack_ecmascript_runtime::embed_fs()
                .root()
                .to_resolved()
                .await?,
        ),
        ContextCondition::InPath(
            turbopack_node::embed_js::embed_fs()
                .root()
                .to_resolved()
                .await?,
        ),
    ]))
}
