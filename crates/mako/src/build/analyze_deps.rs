use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use thiserror::Error;

use crate::ast::error;
use crate::ast::file::File;
use crate::compiler::Context;
use crate::module::{Dependency, ModuleAst};
use crate::resolve::{resolve, ResolverResource};

#[derive(Debug, Error)]
pub enum AnalyzeDepsError {
    #[error("{message:}")]
    ModuleNotFound { message: String },
}

#[derive(Debug, Clone, Default)]
pub struct AnalyzeDepsResult {
    pub resolved_deps: Vec<ResolvedDep>,
    // why use hash map?
    // since we need source as key to replace in generate step
    pub missing_deps: HashMap<String, Dependency>,
}

#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub resolver_resource: ResolverResource,
    pub dependency: Dependency,
}

pub struct AnalyzeDeps {}

impl AnalyzeDeps {
    pub fn analyze_deps(
        ast: &ModuleAst,
        file: &File,
        context: Arc<Context>,
    ) -> Result<AnalyzeDepsResult> {
        crate::mako_profile_function!();
        let mut deps = match ast {
            ModuleAst::Script(ast) => ast.analyze_deps(context.clone()),
            ModuleAst::Css(ast) => ast.analyze_deps(),
            _ => vec![],
        };
        context.plugin_driver.before_resolve(&mut deps, &context)?;
        Self::check_deps(&deps, file)?;

        let mut resolved_deps = vec![];
        let mut missing_deps = HashMap::new();

        for dep in deps {
            let result = resolve(
                &file.resolve_from(&context),
                &dep,
                &context.resolvers,
                &context,
            );
            match result {
                Ok(resolver_resource) => {
                    let resolved_dep = ResolvedDep {
                        resolver_resource,
                        dependency: dep,
                    };
                    context
                        .plugin_driver
                        .after_resolve(&resolved_dep, &context)?;
                    resolved_deps.push(resolved_dep);
                }
                Err(_err) => {
                    missing_deps.insert(dep.source.clone(), dep);
                }
            }
        }

        if !missing_deps.is_empty() {
            let messages = missing_deps
                .values()
                .map(|dep| Self::get_resolved_error(dep, context.clone()))
                .collect::<Vec<String>>()
                .join("\n");
            if context.args.watch {
                eprintln!("{}", messages);
            } else {
                return Err(anyhow!(AnalyzeDepsError::ModuleNotFound {
                    message: messages
                }));
            }
        }

        Ok(AnalyzeDepsResult {
            resolved_deps,
            missing_deps,
        })
    }

    fn check_deps(deps: &Vec<Dependency>, file: &File) -> Result<()> {
        for dep in deps {
            // webpack loader syntax is not supported
            if dep.source.contains("-loader!")
                || (dep.source.contains("-loader?") && dep.source.contains('!'))
            {
                return Err(anyhow!(
                    "webpack loader syntax is not supported, since found dep {:?} in {:?}",
                    dep.source,
                    file.path.to_str().unwrap()
                ));
            }
        }
        Ok(())
    }

    pub fn get_resolved_error(dep: &Dependency, context: Arc<Context>) -> String {
        let message = format!("Module not found: Can't resolve '{}'", dep.source);
        if dep.span.is_some() {
            // TODO: support css resolved error
            error::code_frame(error::ErrorSpan::Js(dep.span.unwrap()), &message, context)
        } else {
            message
        }
    }
}
