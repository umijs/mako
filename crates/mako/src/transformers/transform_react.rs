use std::path::Path;
use std::sync::Arc;

use mako_core::swc_common::comments::NoopComments;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{chain, Mark, SourceMap};
use mako_core::swc_ecma_ast::{Module, ModuleItem, Program};
use mako_core::swc_ecma_transforms_react::{react, Options, RefreshOptions, Runtime};
use mako_core::swc_ecma_visit::{Fold, VisitMut, VisitMutWith};
use mako_core::swc_emotion::{emotion, EmotionOptions};

use crate::ast::build_js_ast;
use crate::compiler::Context;
use crate::config::Mode;
use crate::task::Task;

pub struct PrefixCode {
    pub code: String,
    pub context: Arc<Context>,
}

pub fn mako_react(
    cm: Lrc<SourceMap>,
    context: &Arc<Context>,
    task: &Task,
    top_level_mark: &Mark,
    unresolved_mark: &Mark,
) -> Box<dyn VisitMut> {
    let is_dev = matches!(context.config.mode, Mode::Development);
    let is_browser = matches!(context.config.platform, crate::config::Platform::Browser);
    let use_refresh = is_dev
        && context.args.watch
        && context.config.hmr
        && !task.path.contains("/node_modules/")
        && is_browser;

    let is_jsx = task.path.ends_with(".jsx")
        || task.path.ends_with(".js")
        || task.path.ends_with(".ts")
        || task.path.ends_with(".tsx")
        || task.path.ends_with(".svg");

    if !is_jsx {
        return if task.is_entry && use_refresh {
            Box::new(chain!(react_refresh_inject_runtime_only(context), noop()))
        } else {
            Box::new(noop())
        };
    }

    let (import_source, pragma) = if context.config.emotion {
        ("@emotion/react", "jsx")
    } else {
        ("react", "React.createElement")
    };

    let emotion = if context.config.emotion {
        Box::new(Emotion {
            mode: context.config.mode.clone(),
            cm: cm.clone(),
            path: task.path.clone(),
        })
    } else {
        noop()
    };

    let visit = chain!(
        emotion,
        react(
            cm,
            Some(NoopComments),
            Options {
                import_source: Some(import_source.to_string()),
                pragma: Some(pragma.into()),
                pragma_frag: Some("React.Fragment".into()),
                // support react 17 + only
                runtime: Some(Runtime::Automatic),
                development: Some(is_dev),
                // to avoid throw error for svg namespace element
                throw_if_namespace: Some(false),
                refresh: if use_refresh {
                    Some(RefreshOptions::default())
                } else {
                    None
                },
                ..Default::default()
            },
            *top_level_mark,
            *unresolved_mark,
        )
    );
    if use_refresh {
        Box::new(chain!(
            visit,
            react_refresh_module_prefix(context),
            react_refresh_module_postfix(context)
        ))
    } else {
        Box::new(visit)
    }
}

struct Emotion {
    cm: Lrc<SourceMap>,
    path: String,
    mode: Mode,
}

impl VisitMut for Emotion {
    fn visit_mut_program(&mut self, program: &mut Program) {
        let is_dev = matches!(self.mode, Mode::Development);
        let span = match program {
            Program::Module(module) => module.span,
            Program::Script(script) => script.span,
        };
        let pos = self.cm.lookup_char_pos(span.lo);
        let hash = pos.file.src_hash as u32;
        let mut folder = emotion(
            EmotionOptions {
                enabled: Some(true),
                sourcemap: Some(true),
                auto_label: Some(is_dev),
                import_map: None,
                ..Default::default()
            },
            Path::new(&self.path),
            hash,
            self.cm.clone(),
            NoopComments,
        );
        *program = folder.fold_program(program.clone());
        program.visit_mut_children_with(self);
    }
}

impl VisitMut for PrefixCode {
    fn visit_mut_program(&mut self, program: &mut Program) {
        let post_code_snippet_module =
            build_js_ast("_pre_code.js", &self.code, &self.context).unwrap();
        let stmts = post_code_snippet_module
            .ast
            .as_script()
            .unwrap()
            .body
            .clone();
        match program {
            Program::Module(module) => {
                let module_items = stmts
                    .iter()
                    .map(|f| ModuleItem::Stmt(f.clone()))
                    .collect::<Vec<ModuleItem>>();
                module.body.splice(0..0, module_items);
            }
            Program::Script(script) => {
                script.body.splice(0..0, stmts);
            }
        }
        program.visit_mut_children_with(self);
    }
}

pub struct PostfixCode {
    code: String,
    context: Arc<Context>,
}

impl VisitMut for PostfixCode {
    fn visit_mut_program(&mut self, program: &mut Program) {
        let post_code_snippet_module =
            build_js_ast("_post_code.js", &self.code, &self.context).unwrap();
        let stmts = post_code_snippet_module
            .ast
            .as_script()
            .unwrap()
            .body
            .clone();
        match program {
            Program::Module(module) => {
                let module_items = stmts
                    .iter()
                    .map(|f| ModuleItem::Stmt(f.clone()))
                    .collect::<Vec<ModuleItem>>();
                module.body.extend(module_items);
            }
            Program::Script(script) => {
                script.body.extend(stmts);
            }
        }
        program.visit_mut_children_with(self);
    }
}

pub fn react_refresh_module_prefix(context: &std::sync::Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
import * as RefreshRuntime from 'react-refresh';
var prevRefreshReg;
var prevRefreshSig;

prevRefreshReg = self.$RefreshReg$;
prevRefreshSig = self.$RefreshSig$;
self.$RefreshReg$ = (type, id) => {
  RefreshRuntime.register(type, module.id + id);
};
self.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;
"#
        .to_string(),
    })
}

pub fn react_refresh_inject_runtime_only(context: &std::sync::Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PrefixCode {
        context: context.clone(),
        code: r#"
import 'react-refresh';
"#
        .to_string(),
    })
}

pub fn react_refresh_module_postfix(context: &Arc<Context>) -> Box<dyn VisitMut> {
    Box::new(PostfixCode {
        context: context.clone(),
        code: r#"
self.$RefreshReg$ = prevRefreshReg;
self.$RefreshSig$ = prevRefreshSig;
function $RefreshIsReactComponentLike$(moduleExports) {
  if (RefreshRuntime.isLikelyComponentType(moduleExports.default || moduleExports)) {
    return true;
  }
  for (var key in moduleExports) {
    if (RefreshRuntime.isLikelyComponentType(moduleExports[key])) {
      return true;
    }
  }
  return false;
}
if ($RefreshIsReactComponentLike$(module.exports)) {
    module.meta.hot.accept();
    RefreshRuntime.performReactRefresh();
}
"#
        .to_string(),
    })
}

struct NoopVisitor;

impl VisitMut for NoopVisitor {
    fn visit_mut_module(&mut self, _: &mut Module) {
        // Do nothing.
    }
}

fn noop() -> Box<dyn VisitMut> {
    Box::new(NoopVisitor)
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use mako_core::swc_common::{chain, Mark, GLOBALS};
    use mako_core::swc_ecma_transforms::resolver;
    use mako_core::swc_ecma_visit::VisitMut;

    use crate::assert_display_snapshot;
    use crate::ast::build_js_ast;
    use crate::compiler::{Args, Context};
    use crate::task::{Task, TaskType};
    use crate::test_helper::transform_ast_with;
    use crate::transformers::transform_react::mako_react;

    struct TransformTask {
        code: String,
        path: String,
        is_entry: bool,
    }

    #[test]
    pub fn entry_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            is_entry: true,
            path: "index.js".to_string(),
            code: "console.log('entry');".to_string()
        }));
    }

    #[test]
    pub fn node_modules_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            code: "console.log('in node modules');".to_string(),
            is_entry: false,
            path: "project/node_modules/pkg/index.js".to_string()
        }));
    }

    #[test]
    pub fn normal_module_with_react_refresh() {
        assert_display_snapshot!(transform(TransformTask {
            code: "export default function R(){return <h1></h1>}".to_string(),
            is_entry: false,
            path: "index.jsx".to_string()
        }));
    }

    #[test]
    pub fn svgr_with_namespace() {
        assert_display_snapshot!(transform(TransformTask {
            // part of jsoneditor/dist/img/jsoneditor-icons.svg
            code: r#"const SvgComponent = (props) => (
    <svg
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:cc="http://creativecommons.org/ns#"
        xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
        xmlns:svg="http://www.w3.org/2000/svg"
        xmlns="http://www.w3.org/2000/svg"
        xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
        xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
        width={240}
        height={144}
        id="svg4136"
        inkscape:version="0.91 r13725"
        sodipodi:docname="jsoneditor-icons.svg"
        {...props}
    >
        <metadata id="metadata4148">
            <rdf:RDF></rdf:RDF>
        </metadata>
    </svg>
)"#
            .to_string(),
            is_entry: false,
            path: "index.jsx".to_string()
        }));
    }

    fn transform(task: TransformTask) -> String {
        let context: Arc<Context> = Arc::new(Context {
            args: Args { watch: true },
            ..Default::default()
        });

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("index.jsx", &task.code, &context).unwrap();

            let task_type = if task.is_entry {
                TaskType::Entry(task.path.clone())
            } else {
                TaskType::Normal(task.path.clone())
            };
            let mut visitor: Box<dyn VisitMut> = Box::new(chain!(
                resolver(Mark::new(), Mark::new(), false),
                mako_react(
                    Default::default(),
                    &context,
                    &Task::new(task_type, None),
                    &Mark::new(),
                    &Mark::new(),
                )
            ));

            transform_ast_with(&mut ast.ast, &mut visitor, &context.meta.script.cm)
        })
    }
}
