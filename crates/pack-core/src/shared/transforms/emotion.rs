use anyhow::Result;
use turbo_tasks::Vc;
use turbopack::module_options::ModuleRule;
use turbopack_ecmascript_plugins::transform::emotion::EmotionTransformer;

use super::get_ecma_transform_rule;
use crate::config::{Config, EmotionTransformOptionsOrBoolean};

pub async fn get_emotion_transform_rule(config: Vc<Config>) -> Result<Option<ModuleRule>> {
    let enable_mdx_rs = config.mdx_rs().await?.is_some();
    let module_rule = config
        .styles()
        .await?
        .emotion
        .as_ref()
        .and_then(|config| match config {
            EmotionTransformOptionsOrBoolean::Boolean(true) => {
                EmotionTransformer::new(&Default::default())
            }
            EmotionTransformOptionsOrBoolean::Options(value) => EmotionTransformer::new(value),
            _ => None,
        })
        .map(|transformer| get_ecma_transform_rule(Box::new(transformer), enable_mdx_rs, true));

    Ok(module_rule)
}
