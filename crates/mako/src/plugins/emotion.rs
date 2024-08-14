use std::path::Path;

use anyhow::Ok;
use swc_core::common::comments::NoopComments;
use swc_core::common::sync::Lrc;
use swc_core::common::SourceMap;
use swc_core::ecma::ast::Module;
use swc_core::ecma::visit::{Fold, VisitMut, VisitMutWith};
use swc_emotion::{emotion, EmotionOptions};

use crate::config::{Mode, ReactConfig};
use crate::plugin::Plugin;

pub struct EmotionPlugin {}

impl Plugin for EmotionPlugin {
    fn name(&self) -> &str {
        "emotion"
    }

    fn modify_config(
        &self,
        config: &mut crate::config::Config,
        _root: &std::path::Path,
        _args: &crate::compiler::Args,
    ) -> anyhow::Result<()> {
        if config.emotion {
            config.react = ReactConfig {
                pragma: "jsx".into(),
                import_source: "@emotion/react".into(),
                pragma_frag: config.react.pragma_frag.clone(),
                runtime: config.react.runtime.clone(),
            }
        }
        Ok(())
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        ast: &mut swc_core::ecma::ast::Module,
        context: &std::sync::Arc<crate::compiler::Context>,
    ) -> anyhow::Result<()> {
        if context.config.emotion {
            ast.visit_mut_with(&mut Emotion {
                mode: context.config.mode.clone(),
                cm: context.meta.script.cm.clone(),
                path: param.path.into(),
            });
        }

        Ok(())
    }
}

struct Emotion {
    cm: Lrc<SourceMap>,
    path: String,
    mode: Mode,
}

impl VisitMut for Emotion {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let is_dev = matches!(self.mode, Mode::Development);
        let pos = self.cm.lookup_char_pos(module.span.lo);
        let hash = pos.file.src_hash as u32;
        let mut folder = emotion(
            EmotionOptions {
                enabled: Some(true),
                sourcemap: Some(is_dev),
                auto_label: Some(is_dev),
                import_map: None,
                ..Default::default()
            },
            Path::new(&self.path),
            hash,
            self.cm.clone(),
            NoopComments,
        );
        module.body = folder.fold_module(module.clone()).body;

        module.visit_mut_children_with(self);
    }
}
