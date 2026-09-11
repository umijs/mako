use std::sync::Arc;

use anyhow::{Context, Result, bail};
use swc_core::{
    base::try_with_handler,
    common::{
        EqIgnoreSpan, FileName, FilePathMapping, GLOBALS, Mark, SourceMap,
        comments::{Comments, SingleThreadedComments},
    },
    ecma::{
        ast::{EsVersion, Program},
        codegen::{Emitter, text_writer::JsWriter},
        parser::{Parser, StringInput, Syntax, lexer::Lexer},
        preset_env::{Config, Targets, transform_from_env},
        transforms::base::{
            assumptions::Assumptions,
            fixer::fixer,
            helpers::{HELPERS, Helpers, inject_helpers},
            hygiene::{self, hygiene_with_config},
            resolver,
        },
        visit::VisitWith,
    },
};
use turbo_tasks::Vc;
use turbo_tasks_fs::rope::Rope;
use turbopack_core::{
    code_builder::{Code, CodeBuilder},
    environment::Environment,
};
use turbopack_ecmascript::parse::{IdentCollector, generate_js_source_map};

pub(super) enum CodegenStage {
    Lower,
    EmitMinified,
}

/// Module transforms run before code generation. Lower the complete library as well so that
/// runtime assets, module factories and generated async-module wrappers obey the same target.
pub(super) async fn generate_library_code(
    code: Code,
    environment: Vc<Environment>,
    source_maps: bool,
    stage: CodegenStage,
) -> Result<Code> {
    let versions = *environment.runtime_versions().await?;
    let target = if *environment
        .runtime_versions()
        .supports_arrow_functions()
        .await?
    {
        EsVersion::latest()
    } else {
        EsVersion::Es5
    };
    let lower = matches!(stage, CodegenStage::Lower);
    let source = code.source_code().to_str()?.into_owned();
    let cm = Arc::new(SourceMap::new(FilePathMapping::empty()));
    let fm = cm.new_source_file(FileName::Anon.into(), source);
    let comments = SingleThreadedComments::default();
    let lexer = Lexer::new(
        Syntax::default(),
        EsVersion::latest(),
        StringInput::from(&*fm),
        Some(&comments),
    );
    let mut parser = Parser::new_from(lexer);

    let transformed = try_with_handler(cm.clone(), Default::default(), |handler| {
        GLOBALS.set(&Default::default(), || {
            let mut program = match parser.parse_program() {
                Ok(program) => program,
                Err(error) => {
                    error.into_diagnostic(handler).emit();
                    bail!("failed to parse library output");
                }
            };
            let errors = parser.take_errors();
            if !errors.is_empty() {
                for error in errors {
                    error.into_diagnostic(handler).emit();
                }
                bail!("failed to parse library output");
            }
            let names = if source_maps {
                let mut collector = IdentCollector::default();
                program.visit_with(&mut collector);
                collector.into_map()
            } else {
                Default::default()
            };

            let unresolved_mark = Mark::new();
            let top_level_mark = Mark::new();
            program.mutate(resolver(unresolved_mark, top_level_mark, false));
            // Compare after resolving bindings, before hygiene/fixer can change formatting.
            // ES5 still needs emission to normalize raw literals and statement separators.
            let original_program = (lower && target != EsVersion::Es5).then(|| program.clone());
            // Library output is self-contained: new transform helpers cannot be imports.
            let has_helpers = if lower {
                HELPERS.set(&Helpers::new(false), || {
                    program.mutate(transform_from_env::<&dyn Comments>(
                        unresolved_mark,
                        Some(&comments),
                        Config {
                            targets: Some(Targets::Versions(versions)),
                            ..Default::default()
                        }
                        .into(),
                        Assumptions::default(),
                    ));
                    let statements_before = statement_count(&program);
                    program.mutate(inject_helpers(unresolved_mark));
                    statement_count(&program) > statements_before
                })
            } else {
                false
            };
            if original_program
                .as_ref()
                .is_some_and(|original| program.eq_ignore_span(original))
            {
                return Ok(None);
            }
            program.mutate(hygiene_with_config(hygiene::Config {
                top_level_mark,
                ..Default::default()
            }));
            program.mutate(fixer(Some(&comments)));
            Ok(Some((program, names, has_helpers)))
        })
    })
    .map_err(|error| error.to_pretty_error())?;

    let Some((program, names, has_helpers)) = transformed else {
        // Preserve the original chunk layout, comments and sectioned source map when no
        // compatibility transform was needed. Reprinting would only add debugging noise.
        return Ok(code);
    };
    let original_map = source_maps.then(|| code.generate_source_map_ref(None));
    let generate_debug_id = code.should_generate_debug_id();

    let mut source = Vec::new();
    let mut mappings = Vec::new();
    Emitter {
        cfg: swc_core::ecma::codegen::Config::default()
            .with_target(target)
            .with_minify(!lower),
        comments: Some(&comments),
        cm: cm.clone(),
        wr: JsWriter::new(
            cm.clone(),
            "\n",
            &mut source,
            source_maps.then_some(&mut mappings),
        ),
    }
    .emit_program(&program)
    .context("failed to emit library output")?;

    let source: Rope = String::from_utf8(source)?.into();
    let mut builder = CodeBuilder::new(source_maps, generate_debug_id);
    // SWC inserts helpers at program scope. Keep them private to this library, including the
    // module factories passed as arguments to its runtime IIFE.
    if has_helpers {
        builder += "(function() {\n";
    }
    if let Some(original_map) = &original_map {
        builder.push_source(
            &source,
            Some(generate_js_source_map(
                &*cm,
                mappings,
                Some(original_map),
                true,
                false,
                names,
            )?),
        );
    } else {
        builder.push_source(&source, None::<Rope>);
    }
    if has_helpers {
        builder += "\n}).call(this);\n";
    }
    Ok(builder.build())
}

fn statement_count(program: &Program) -> usize {
    match program {
        Program::Module(module) => module.body.len(),
        Program::Script(script) => script.body.len(),
    }
}
