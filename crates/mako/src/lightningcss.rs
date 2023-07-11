use std::sync::Arc;

use lightningcss::stylesheet::{
    MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet,
};

use crate::compiler::Context;
use crate::config::Mode;
use crate::targets;

pub fn lightingcss_transform(code: &str, context: &Arc<Context>) -> String {
    // something more, lightning will transform @import url() to @import ""
    let targets = targets::lightningcss_targets_from_map(context.config.targets.clone());
    let flags = ParserFlags::empty();
    // flags.set(ParserFlags::NESTING, true);
    let mut lightingcss_stylesheet = StyleSheet::parse(
        code,
        ParserOptions {
            flags,
            ..Default::default()
        },
    )
    .unwrap();
    lightingcss_stylesheet
        .minify(MinifyOptions {
            targets,
            ..Default::default()
        })
        .unwrap();
    let out = lightingcss_stylesheet
        .to_css(PrinterOptions {
            minify: matches!(context.config.mode, Mode::Production),
            targets,
            ..Default::default()
        })
        .unwrap();
    out.code
}
