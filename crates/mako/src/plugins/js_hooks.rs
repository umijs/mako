use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::serde_xml_rs::from_str as from_xml_str;

use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::plugin::Plugin;

pub struct JsHooksPlugin {}

impl Plugin for JsHooksPlugin {
    fn name(&self) -> &str {
        "js_hooks"
    }
}
