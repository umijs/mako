use std::mem::take;

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;
use turbopack_node::transforms::webpack::WebpackLoaderItem;

pub fn style_loader(inline_css: &JsonValue) -> Result<WebpackLoaderItem> {
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
