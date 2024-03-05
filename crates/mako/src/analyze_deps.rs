use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::thiserror::Error;

use crate::ast_2::error;
use crate::ast_2::file::File;
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
    pub missing_deps: Vec<Dependency>,
    pub ignored_deps: Vec<Dependency>,
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
        mako_core::mako_profile_function!();
        let deps = match ast {
            ModuleAst::Script(ast) => ast.analyze_deps(context.clone()),
            ModuleAst::Css(ast) => ast.analyze_deps(),
            _ => vec![],
        };
        Self::check_deps(&deps, file)?;

        let mut resolved_deps = vec![];
        let mut missing_deps = vec![];
        let mut ignored_deps = vec![];
        let path = file.path.to_str().unwrap();
        for dep in deps {
            let result = resolve(
                // .
                path,
                &dep,
                &context.resolvers,
                &context,
            );
            match result {
                Ok(resolver_resource) => {
                    if matches!(resolver_resource, ResolverResource::Ignored) {
                        ignored_deps.push(dep);
                    } else {
                        resolved_deps.push(ResolvedDep {
                            resolver_resource,
                            dependency: dep,
                        });
                    }
                }
                Err(_err) => {
                    missing_deps.push(dep);
                }
            }
        }

        if !missing_deps.is_empty() {
            let messages = missing_deps
                .iter()
                .map(|dep| Self::get_resolved_error(dep, context.clone()))
                .collect::<Vec<String>>()
                .join("\n");
            // TODO:
            // should we just throw an error here and decide whether to print or exit at the upper level?
            if context.args.watch {
                eprint!("{}", messages);
            } else {
                return Err(anyhow!(AnalyzeDepsError::ModuleNotFound {
                    message: messages
                }));
            }
        }

        Ok(AnalyzeDepsResult {
            resolved_deps,
            missing_deps,
            ignored_deps,
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
            error::code_frame(dep.span.unwrap(), &message, context)
        } else {
            message
        }
    }
}
