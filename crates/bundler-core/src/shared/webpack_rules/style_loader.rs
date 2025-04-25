use std::mem::take;

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;
use turbopack_node::transforms::webpack::WebpackLoaderItem;

use turbo_tasks::{ResolvedVc, Vc};
use turbopack::module_options::{LoaderRuleItem, OptionWebpackRules, WebpackRules};

use crate::config::OptionalJsonValue;

fn style_loader(inline_css: &JsonValue) -> Result<WebpackLoaderItem> {
    let Some(inline_css) = inline_css.as_object().cloned() else {
        bail!("inline_css must be an object");
    };

    let style_loader = WebpackLoaderItem {
        loader: "@utoo/style-loader".into(),
        options: take(
            serde_json::json!({
                "insert": inline_css.get("insert"),
                "injectType": inline_css.get("injectType"),
            })
            .as_object_mut()
            .unwrap(),
        ),
    };

    Ok(style_loader)
}

#[turbo_tasks::function]
pub async fn maybe_add_style_loader(
    inline_css: Vc<OptionalJsonValue>,
    webpack_rules: Option<Vc<WebpackRules>>,
) -> Result<Vc<OptionWebpackRules>> {
    let mut rules = if let Some(webpack_rules) = webpack_rules {
        webpack_rules.owned().await?
    } else {
        Default::default()
    };

    let Some(inline_css) = &*inline_css.await? else {
        return Ok(Vc::cell(Some(ResolvedVc::cell(rules))));
    };
    let Some(_) = inline_css.as_object().cloned() else {
        bail!("inline_css must be an object");
    };

    for (pattern, rename) in [("*.css", ".js")] {
        let rule = rules.get_mut(pattern);
        let style_loader = style_loader(inline_css)?;

        if let Some(rule) = rule {
            // Without `as`, loader result would be JS code, so we don't want to apply
            // style-loader on that.
            let Some(rename_as) = rule.rename_as.as_ref() else {
                continue;
            };
            // Only when the result should run through the style pipeline, we apply
            // style-loader.

            if rename_as != "*" {
                continue;
            }
            let mut loaders = rule.loaders.owned().await?;
            loaders.push(style_loader);
            rule.loaders = ResolvedVc::cell(loaders);
        } else {
            rules.insert(
                pattern.into(),
                LoaderRuleItem {
                    loaders: ResolvedVc::cell(vec![style_loader]),
                    rename_as: Some(format!("*{rename}").into()),
                },
            );
        }
    }

    Ok(Vc::cell(Some(ResolvedVc::cell(rules))))
}
