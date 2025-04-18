use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbopack::module_options::WebpackLoadersOptions;
use turbopack_core::resolve::options::ImportMapping;

use self::less::maybe_add_less_loader;
use self::sass::maybe_add_sass_loader;
use crate::config::Config;

pub(crate) mod less;
pub(crate) mod sass;

pub async fn webpack_loader_options(
    config: Vc<Config>,
    conditions: Vec<RcStr>,
) -> Result<Option<ResolvedVc<WebpackLoadersOptions>>> {
    let rules = *config.webpack_rules(conditions).await?;
    let rules = *maybe_add_sass_loader(config.sass_config(), rules.map(|v| *v)).await?;
    let rules = *maybe_add_less_loader(config.less_config(), rules.map(|v| *v)).await?;

    Ok(if let Some(rules) = rules {
        Some(
            WebpackLoadersOptions {
                rules,
                loader_runner_package: Some(loader_runner_package_mapping().to_resolved().await?),
            }
            .resolved_cell(),
        )
    } else {
        None
    })
}

#[turbo_tasks::function]
async fn loader_runner_package_mapping() -> Result<Vc<ImportMapping>> {
    Ok(
        ImportMapping::Alternatives(vec![ImportMapping::PrimaryAlternative(
            "loader-runner".into(),
            None,
        )
        .resolved_cell()])
        .cell(),
    )
}
