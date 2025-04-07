use anyhow::Result;
use turbo_tasks::Vc;
use turbopack::module_options::ModuleRule;

use crate::{config::Config, shared::transforms::modularize_imports::get_modularize_imports_rule};

/// Returns a list of module rules which apply client-side, Next.js-specific
/// transforms.
pub async fn get_client_transforms_rules(config: Vc<Config>) -> Result<Vec<ModuleRule>> {
    let mut rules = vec![];

    let modularize_imports_config = &config.modularize_imports().await?;
    let enable_mdx_rs = config.mdx_rs().await?.is_some();

    if !modularize_imports_config.is_empty() {
        rules.push(get_modularize_imports_rule(
            modularize_imports_config,
            enable_mdx_rs,
        ));
    }

    Ok(rules)
}
