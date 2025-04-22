use std::mem::take;

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;
use turbopack_node::transforms::webpack::WebpackLoaderItem;

pub fn style_loader(style_options: &JsonValue) -> Result<WebpackLoaderItem> {
    let Some(style_options) = style_options.as_object().cloned() else {
        bail!("style_options must be an object");
    };

    let style_loader = WebpackLoaderItem {
        loader: "@utoo/style-loader".into(),
        options: take(
            serde_json::json!({
                "insert": style_options.get("insert"),
                "injectType": style_options.get("injectType"),
            })
            .as_object_mut()
            .unwrap(),
        ),
    };

    Ok(style_loader)
}
