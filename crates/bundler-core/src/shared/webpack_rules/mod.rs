use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack::module_options::WebpackLoadersOptions;
use turbopack_core::resolve::options::ImportMapping;

use self::less::maybe_add_less_loader;
use self::sass::maybe_add_sass_loader;
use crate::config::Config;
use crate::import_map::get_bundler_package;

pub(crate) mod less;
pub(crate) mod sass;
pub(crate) mod style_loader;

pub async fn webpack_loader_options(
    project_path: ResolvedVc<FileSystemPath>,
    config: Vc<Config>,
    conditions: Vec<RcStr>,
) -> Result<Option<ResolvedVc<WebpackLoadersOptions>>> {
    let rules = *config.webpack_rules(conditions).await?;
    let rules = *maybe_add_sass_loader(
        config.sass_config(),
        config.style_options(),
        rules.map(|v| *v),
    )
    .await?;
    let rules = *maybe_add_less_loader(
        config.less_config(),
        config.style_options(),
        rules.map(|v| *v),
    )
    .await?;

    Ok(if let Some(rules) = rules {
        Some(
            WebpackLoadersOptions {
                rules,
                loader_runner_package: Some(
                    loader_runner_package_mapping(*project_path)
                        .to_resolved()
                        .await?,
                ),
            }
            .resolved_cell(),
        )
    } else {
        None
    })
}

#[turbo_tasks::function]
async fn loader_runner_package_mapping(
    project_path: ResolvedVc<FileSystemPath>,
) -> Result<Vc<ImportMapping>> {
    Ok(
        ImportMapping::Alternatives(vec![ImportMapping::PrimaryAlternative(
            "@utoo/loader-runner".into(),
            Some(get_bundler_package(*project_path).to_resolved().await?),
        )
        .resolved_cell()])
        .cell(),
    )
}
