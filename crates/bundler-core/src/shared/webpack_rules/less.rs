use std::mem::take;

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;
use turbo_tasks::{ResolvedVc, Vc};
use turbopack::module_options::{LoaderRuleItem, OptionWebpackRules, WebpackRules};
use turbopack_node::transforms::webpack::WebpackLoaderItem;

use crate::config::OptionalJsonValue;

use super::style_loader::style_loader;

#[turbo_tasks::function]
pub async fn maybe_add_less_loader(
    less_options: Vc<JsonValue>,
    style_options: Vc<OptionalJsonValue>,
    webpack_rules: Option<Vc<WebpackRules>>,
) -> Result<Vc<OptionWebpackRules>> {
    let less_options = less_options.await?;
    let Some(less_options) = less_options.as_object().cloned() else {
        bail!("less_options must be an object");
    };

    let mut rules = if let Some(webpack_rules) = webpack_rules {
        webpack_rules.owned().await?
    } else {
        Default::default()
    };
    for (pattern, rename) in [("*.module.less", ".module.css"), ("*.less", ".css")] {
        let style_options = &*style_options.await?;

        let rename = if style_options.is_some() {
            format!("{}.js", rename)
        } else {
            rename.to_string()
        };

        // additionalData is a loader option but Next.js has it under `lessOptions` in
        // `config.js`
        let empty_additional_data = serde_json::Value::String("".to_string());
        let additional_data = less_options.get("prependData").or(less_options
            .get("additionalData")
            .or(Some(&empty_additional_data)));
        let rule = rules.get_mut(pattern);
        let less_loader = WebpackLoaderItem {
            loader: "less-loader".into(),
            options: take(
                serde_json::json!({
                    "implementation": less_options.get("implementation"),
                    "sourceMap": true,
                    "lessOptions": less_options,
                    "additionalData": additional_data
                })
                .as_object_mut()
                .unwrap(),
            ),
        };

        if let Some(rule) = rule {
            // Without `as`, loader result would be JS code, so we don't want to apply
            // less-loader on that.
            let Some(rename_as) = rule.rename_as.as_ref() else {
                continue;
            };
            // Only when the result should run through the less pipeline, we apply
            // less-loader.

            if rename_as != "*" {
                continue;
            }
            let mut loaders = rule.loaders.owned().await?;
            if let Some(style_options) = style_options {
                loaders.push(style_loader(style_options)?);
            }
            loaders.push(less_loader);
            rule.loaders = ResolvedVc::cell(loaders);
        } else {
            let loaders = if let Some(style_options) = style_options {
                vec![style_loader(style_options)?, less_loader]
            } else {
                vec![less_loader]
            };
            rules.insert(
                pattern.into(),
                LoaderRuleItem {
                    loaders: ResolvedVc::cell(loaders),
                    rename_as: Some(format!("*{rename}").into()),
                },
            );
        }
    }

    Ok(Vc::cell(Some(ResolvedVc::cell(rules))))
}
