use anyhow::Result;
use style_loader::maybe_add_style_loader;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack::module_options::WebpackLoadersOptions;
use turbopack_core::resolve::{ExternalTraced, ExternalType, options::ImportMapping};

use self::less::maybe_add_less_loader;
use self::sass::maybe_add_sass_loader;
use crate::config::Config;

pub(crate) mod less;
pub(crate) mod sass;
pub(crate) mod style_loader;

pub async fn webpack_loader_options(
    project_path: FileSystemPath,
    config: Vc<Config>,
    conditions: Vec<RcStr>,
) -> Result<Option<ResolvedVc<WebpackLoadersOptions>>> {
    let rules = *config.webpack_rules(conditions).await?;
    let rules = *maybe_add_style_loader(config.inline_css(), rules.map(|v| *v)).await?;
    let rules = *maybe_add_less_loader(config.less_config(), rules.map(|v| *v)).await?;
    let rules = *maybe_add_sass_loader(config.sass_config(), rules.map(|v| *v)).await?;

    Ok(if let Some(rules) = rules {
        Some(
            WebpackLoadersOptions {
                rules,
                // TODO: https://github.com/vercel/next.js/pull/78733
                conditions: ResolvedVc::cell(None),
                loader_runner_package: Some(
                    loader_runner_package_mapping(project_path)
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
async fn loader_runner_package_mapping(_project_path: FileSystemPath) -> Result<Vc<ImportMapping>> {
    Ok(ImportMapping::Alternatives(vec![
        ImportMapping::External(
            Some("@utoo/loader-runner".into()),
            ExternalType::CommonJs,
            ExternalTraced::Untraced,
        )
        .resolved_cell(),
    ])
    .cell())
}
