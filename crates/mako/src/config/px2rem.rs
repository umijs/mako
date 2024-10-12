use serde::{Deserialize, Serialize};

use crate::{create_deserialize_fn, visitors};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Px2RemConfig {
    #[serde(default = "visitors::css_px2rem::default_root")]
    pub root: f64,
    #[serde(rename = "propBlackList", default)]
    pub prop_blacklist: Vec<String>,
    #[serde(rename = "propWhiteList", default)]
    pub prop_whitelist: Vec<String>,
    #[serde(rename = "selectorBlackList", default)]
    pub selector_blacklist: Vec<String>,
    #[serde(rename = "selectorWhiteList", default)]
    pub selector_whitelist: Vec<String>,
    #[serde(rename = "selectorDoubleList", default)]
    pub selector_doublelist: Vec<String>,
    #[serde(rename = "minPixelValue", default)]
    pub min_pixel_value: f64,
    #[serde(rename = "mediaQuery", default)]
    pub media_query: bool,
}

impl Default for Px2RemConfig {
    fn default() -> Self {
        Px2RemConfig {
            root: visitors::css_px2rem::default_root(),
            prop_blacklist: vec![],
            prop_whitelist: vec![],
            selector_blacklist: vec![],
            selector_whitelist: vec![],
            selector_doublelist: vec![],
            min_pixel_value: 0.0,
            media_query: false,
        }
    }
}

create_deserialize_fn!(deserialize_px2rem, Px2RemConfig);
