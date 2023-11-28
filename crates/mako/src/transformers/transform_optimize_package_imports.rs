use std::path::PathBuf;
use std::sync::Arc;

use cached::proc_macro::cached;
use mako_core::anyhow::Result;
use mako_core::nodejs_resolver::DescriptionData;
use mako_core::path_clean::PathClean;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    ExportSpecifier, Expr, ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier,
    ImportSpecifier, ImportStarAsSpecifier, ModuleDecl, ModuleExportName, ModuleItem, Stmt,
};
use mako_core::swc_ecma_utils::{quote_ident, quote_str};
use mako_core::swc_ecma_visit::Fold;
use mako_core::tracing::debug;

use crate::build::parse_path;
use crate::compiler::Context;
use crate::load::load;
use crate::module::{Dependency, ResolveType};
use crate::parse::parse;
use crate::resolve::{resolve, ResolverResource};

pub fn optimize_package_imports(path: String, context: Arc<Context>) -> impl Fold {
    OptimizePackageImports { path, context }
}

struct OptimizePackageImports {
    path: String,
    context: Arc<Context>,
}

impl Fold for OptimizePackageImports {
    fn fold_module_items(&mut self, module_items: Vec<ModuleItem>) -> Vec<ModuleItem> {
        let mut new_items = vec![];

        for module_item in module_items {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = &module_item {
                // 1. Exclude situations where import source does not need to be replaced:
                //   - namespace import
                //   - named import with non specifier
                // TODO: Consider support import default?
                if import_decl
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.is_namespace() || specifier.is_default())
                {
                    new_items.push(module_item);
                    continue;
                }

                // specifiers with type ImportNamedSpecifier
                let specifiers = import_decl
                    .specifiers
                    .iter()
                    .map(|specifier| specifier.as_named().unwrap())
                    .collect::<Vec<_>>();

                if specifiers.is_empty() {
                    new_items.push(module_item);
                    continue;
                }

                // 2. resolve dep, and then determine whether it is a barrel file
                let raw_src = import_decl.src.value.to_string();
                let dep = Dependency {
                    source: raw_src.clone(),
                    resolve_as: None,
                    resolve_type: ResolveType::Import,
                    order: 0,
                    span: None,
                };

                match resolve(&self.path, &dep, &self.context.resolvers, &self.context) {
                    Ok(resolved_resource) => {
                        if let ResolverResource::Resolved(resolved) = &resolved_resource {
                            // package with side-effects are not allowed
                            let side_effects = if let Some(description) = &resolved.0.description {
                                has_side_effects(description)
                            } else {
                                true
                            };

                            if side_effects {
                                new_items.push(module_item);
                                continue;
                            }

                            // Whether the bucket file
                            let path = resolved_resource.get_resolved_path();
                            let (is_barrel, export_map) =
                                parse_barrel_file(&path, &self.context).unwrap();

                            if !is_barrel {
                                new_items.push(module_item);
                                continue;
                            }

                            debug!(
                                "parsed barrel file: {:?}, export_map:{:?}",
                                path, export_map
                            );

                            // 2. If it's a bucket file, rewrite the source of import statement
                            //   - `import { a } from 'foo';` => `import { a } from 'foo/a';`
                            //   - `import { a, b, bb } from 'foo';` => `import { a } from 'foo/a'; import { b, bb } from 'foo/b';`
                            // src_specifiers_map: [ ('foo/a', [a]), ('foo/b', [b, bb]) ]
                            let mut src_specifiers_map: Vec<(String, Vec<ImportSpecifier>)> =
                                vec![];
                            specifiers.iter().for_each(|specifier| {
                                // import { a, foo as bar } from 'foo'; => a、foo
                                let imported = match &specifier.imported {
                                    Some(n) => match &n {
                                        ModuleExportName::Ident(n) => n.sym.to_string(),
                                        ModuleExportName::Str(n) => n.value.to_string(),
                                    },
                                    None => specifier.local.sym.to_string(),
                                };

                                // If the import specifier is exported from the barrel file, insert to src_specifiers_map
                                if let Some(export) = export_map.iter().find(|export| {
                                    export.exported == imported && export.orig == "*"
                                }) {
                                    // namespace specifier: `export * as foo from 'foo';`
                                    let new_src = PathBuf::from(&path)
                                        .parent()
                                        .unwrap()
                                        .join(&export.src)
                                        .clean()
                                        .to_string_lossy()
                                        .to_string();

                                    match src_specifiers_map
                                        .iter_mut()
                                        .find(|(src, _)| src == &new_src)
                                    {
                                        Some(map) => map.1.push(ImportSpecifier::Namespace(
                                            ImportStarAsSpecifier {
                                                span: DUMMY_SP,
                                                local: specifier.local.clone(),
                                            },
                                        )),
                                        None => {
                                            src_specifiers_map.push((
                                                new_src,
                                                vec![ImportSpecifier::Namespace(
                                                    ImportStarAsSpecifier {
                                                        span: DUMMY_SP,
                                                        local: specifier.local.clone(),
                                                    },
                                                )],
                                            ));
                                        }
                                    }
                                } else if let Some(export) = export_map.iter().find(|export| {
                                    export.exported == imported && export.orig == "default"
                                }) {
                                    // default specifier: `export { default as Button } from 'button';`
                                    let new_src = PathBuf::from(&path)
                                        .parent()
                                        .unwrap()
                                        .join(&export.src)
                                        .clean()
                                        .to_string_lossy()
                                        .to_string();

                                    match src_specifiers_map
                                        .iter_mut()
                                        .find(|(src, _)| src == &new_src)
                                    {
                                        Some(map) => map.1.push(ImportSpecifier::Default(
                                            ImportDefaultSpecifier {
                                                span: DUMMY_SP,
                                                local: specifier.local.clone(),
                                            },
                                        )),
                                        None => {
                                            src_specifiers_map.push((
                                                new_src,
                                                vec![ImportSpecifier::Default(
                                                    ImportDefaultSpecifier {
                                                        span: DUMMY_SP,
                                                        local: specifier.local.clone(),
                                                    },
                                                )],
                                            ));
                                        }
                                    }
                                } else if let Some(export) =
                                    export_map.iter().find(|export| export.exported == imported)
                                {
                                    // 'foo/a'
                                    let new_src = PathBuf::from(&path)
                                        .parent()
                                        .unwrap()
                                        .join(&export.src)
                                        .clean()
                                        .to_string_lossy()
                                        .to_string();

                                    let new_specifier = if export.orig == export.exported {
                                        // `export { a } from 'a';` and `import { a } from 'foo'`
                                        ImportSpecifier::Named((*specifier).clone())
                                    } else {
                                        // `export { a as aa } from 'a';` and `import { aa } from 'foo'`
                                        ImportSpecifier::Named(ImportNamedSpecifier {
                                            span: DUMMY_SP,
                                            local: specifier.local.clone(),
                                            imported: Some(ModuleExportName::Ident(quote_ident!(
                                                export.orig.clone()
                                            ))),
                                            is_type_only: specifier.is_type_only,
                                        })
                                    };

                                    match src_specifiers_map
                                        .iter_mut()
                                        .find(|(src, _)| src == &new_src)
                                    {
                                        Some(map) => map.1.push(new_specifier),
                                        None => {
                                            src_specifiers_map.push((new_src, vec![new_specifier]));
                                        }
                                    }
                                } else {
                                    // If the import specifier is not exported from the barrel file, keep the import statement here first
                                    match src_specifiers_map
                                        .iter_mut()
                                        .find(|(src, _)| src == &raw_src)
                                    {
                                        Some(map) => {
                                            map.1.push(ImportSpecifier::Named((*specifier).clone()))
                                        }
                                        None => {
                                            src_specifiers_map.push((
                                                raw_src.clone(),
                                                vec![ImportSpecifier::Named((*specifier).clone())],
                                            ));
                                        }
                                    }
                                }
                            });

                            for (new_src, specifiers) in src_specifiers_map {
                                new_items.push(ModuleItem::ModuleDecl(ModuleDecl::Import(
                                    ImportDecl {
                                        span: DUMMY_SP,
                                        specifiers,
                                        src: Box::new(quote_str!(new_src)),
                                        type_only: false,
                                        with: None,
                                    },
                                )));
                            }
                        } else {
                            // Don't replace ignored and external
                            new_items.push(module_item);
                            continue;
                        }
                    }
                    Err(_) => new_items.push(module_item),
                }
            } else {
                new_items.push(module_item);
            }
        }

        new_items
    }
}

#[derive(Debug, Clone)]
struct ImportInfo {
    // `bar` in `export { foo as bar } from './foo';`
    imported: String,
    // `./foo` in `export { foo as bar } from './foo';`
    src: String,
    // `foo` in `export { foo as bar } from './foo';`
    orig: String,
}

#[derive(Debug, Clone)]
struct ExportInfo {
    // `bar` in `export { foo as bar } from './foo';`
    exported: String,
    // `./foo` in `export { foo as bar } from './foo';`
    src: String,
    // `foo` in `export { foo as bar } from './foo';`
    orig: String,
}

#[cached(
    key = "String",
    convert = r#"{ format!("{:?}", description.data().name()) }"#
)]
fn has_side_effects(description: &Arc<DescriptionData>) -> bool {
    let pkg_json = description.data().raw();
    if pkg_json.is_object() {
        if let Some(side_effects) = pkg_json.as_object().unwrap().get("sideEffects") {
            match side_effects {
                serde_json::Value::Bool(side_effects) => *side_effects,
                // Temporary support antd by this way
                serde_json::Value::Array(side_effects) => !side_effects
                    .iter()
                    .all(|rule| rule.is_string() && rule.as_str().unwrap().ends_with(".css")),
                _ => true,
            }
        } else {
            true
        }
    } else {
        true
    }
}

#[cached(result = true, key = "String", convert = r#"{ format!("{}", path) }"#)]
fn parse_barrel_file(path: &str, context: &Arc<Context>) -> Result<(bool, Vec<ExportInfo>)> {
    let request = parse_path(path)?;
    let content = load(&request, false, context)?;
    let ast = parse(&content, &request, context)?;
    let ast = ast.as_script();

    // A barrel file to be a file that only exports from other modules.
    // Besides that, lit expressions are allowed as well ("use client", etc.).
    let mut is_barrel = true;
    // Imported meta information. import { a, b as bb } from './foo'; => [(a, './foo', a), (bb, './foo', b)]
    let mut import_map: Vec<ImportInfo> = vec![];
    // Exportd meta information. export { a, b as bb } from './foo'; => [(a, './foo', a), (bb, './foo', b)]
    let mut export_map: Vec<ExportInfo> = vec![];

    for module_item in &ast.body {
        match module_item {
            ModuleItem::ModuleDecl(module_decl) => {
                match module_decl {
                    // import
                    ModuleDecl::Import(import_decl) => {
                        let src = import_decl.src.value.to_string();

                        for specifier in &import_decl.specifiers {
                            match specifier {
                                // e.g. local = foo, imported = None `import { foo } from 'mod.js'`
                                // e.g. local = bar, imported = Some(foo) for `import { foo as bar } from
                                ImportSpecifier::Named(import_named) => {
                                    import_map.push(ImportInfo {
                                        imported: import_named.local.sym.to_string(),
                                        src: src.clone(),
                                        orig: if let Some(imported) = &import_named.imported {
                                            match imported {
                                                ModuleExportName::Ident(n) => n.sym.to_string(),
                                                ModuleExportName::Str(n) => n.value.to_string(),
                                            }
                                        } else {
                                            import_named.local.sym.to_string()
                                        },
                                    });
                                }
                                // e.g. `import * as foo from 'foo'`.
                                ImportSpecifier::Namespace(import_namespace) => {
                                    import_map.push(ImportInfo {
                                        imported: import_namespace.local.sym.to_string(),
                                        src: src.clone(),
                                        orig: "*".to_string(),
                                    })
                                }
                                // e.g. `import foo from 'foo'`
                                ImportSpecifier::Default(import_default) => {
                                    import_map.push(ImportInfo {
                                        imported: import_default.local.sym.to_string(),
                                        src: src.clone(),
                                        orig: "default".to_string(),
                                    });
                                }
                            }
                        }
                    }
                    // export named
                    ModuleDecl::ExportNamed(export_named) => {
                        for specifier in &export_named.specifiers {
                            match specifier {
                                // e.g. `export { foo } from 'foo';`
                                // e.g. `export { foo as bar } from 'foo';`
                                ExportSpecifier::Named(specifier) => {
                                    let orig_str = match &specifier.orig {
                                        ModuleExportName::Ident(n) => n.sym.to_string(),
                                        ModuleExportName::Str(n) => n.value.to_string(),
                                    };
                                    let name_str = match &specifier.exported {
                                        Some(n) => match &n {
                                            ModuleExportName::Ident(n) => n.sym.to_string(),
                                            ModuleExportName::Str(n) => n.value.to_string(),
                                        },
                                        None => orig_str.clone(),
                                    };
                                    if let Some(src) = &export_named.src {
                                        export_map.push(ExportInfo {
                                            exported: name_str.clone(),
                                            src: src.value.to_string(),
                                            orig: orig_str.clone(),
                                        });
                                    } else {
                                        // resolve src from import_map
                                        let export_info = import_map.iter().find_map(|import| {
                                            if import.imported == orig_str {
                                                Some((
                                                    import.src.to_string(),
                                                    import.orig.to_string(),
                                                ))
                                            } else {
                                                None
                                            }
                                        });

                                        export_map.push(ExportInfo {
                                            exported: name_str.clone(),
                                            src: if let Some(export_info) = &export_info {
                                                export_info.0.clone()
                                            } else {
                                                "".to_string()
                                            },
                                            orig: if let Some(export_info) = &export_info {
                                                export_info.1.clone()
                                            } else {
                                                orig_str.clone()
                                            },
                                        });
                                    }
                                }
                                // e.g. `export * as foo from 'foo';`
                                ExportSpecifier::Namespace(specifier) => {
                                    let name_str = match &specifier.name {
                                        ModuleExportName::Ident(n) => n.sym.to_string(),
                                        ModuleExportName::Str(n) => n.value.to_string(),
                                    };
                                    if let Some(src) = &export_named.src {
                                        export_map.push(ExportInfo {
                                            exported: name_str.clone(),
                                            src: src.value.to_string(),
                                            orig: "*".to_string(),
                                        });
                                    }
                                }
                                // e.g. export v from 'mod';
                                ExportSpecifier::Default(_) => {}
                            }
                        }
                    }
                    // ALLOW: `export * from 'foo'`
                    ModuleDecl::ExportAll(_) => {}
                    // NOT ALLOW:
                    // - ExportDecl: `export const foo = 'foo';`
                    // - ExportDefaultDecl: `export default function foo() {};`
                    // - ExportDefaultExpr: `export default foo;`
                    _ => {
                        is_barrel = false;
                        break;
                    }
                }
            }
            ModuleItem::Stmt(stmt) => match stmt {
                Stmt::Expr(stmt_expr) => match &*stmt_expr.expr {
                    Expr::Lit(_) => {}
                    _ => {
                        is_barrel = false;
                        break;
                    }
                },
                _ => {
                    is_barrel = false;
                    break;
                }
            },
        }
    }

    export_map.retain(|export| export.src.starts_with("./") || export.src.starts_with("../"));

    Ok((is_barrel, export_map))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use mako_core::swc_common::{chain, Mark};
    use mako_core::swc_ecma_parser::{EsConfig, Syntax};
    use mako_core::swc_ecma_transforms::resolver;
    use swc_ecma_transforms_testing::test_fixture;
    use testing::fixture;

    use super::optimize_package_imports;
    use crate::compiler::{Compiler, Context};
    use crate::config::Config;

    #[fixture("test/fixture/optimize_package_imports/**/input.js")]
    fn optimize_package_imports_fixture(input: PathBuf) {
        let output = input.parent().unwrap().join("output.js");
        test_fixture(
            self::syntax(),
            &|_tr| {
                let unresolved_mark = Mark::new();
                let top_level_mark = Mark::new();

                chain!(
                    resolver(unresolved_mark, top_level_mark, false),
                    optimize_package_imports(
                        input.to_string_lossy().to_string(),
                        self::context(&input)
                    ),
                )
            },
            &input,
            &output,
            Default::default(),
        );
    }

    fn syntax() -> Syntax {
        Syntax::Es(EsConfig {
            jsx: true,
            ..Default::default()
        })
    }

    fn context(input: &Path) -> Arc<Context> {
        let root = input.parent().unwrap().to_path_buf();
        let config = Config::new(&root, None, None).unwrap();
        let compiler = Compiler::new(config, root, Default::default(), None).unwrap();
        compiler.context
    }
}
