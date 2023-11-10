use std::path::PathBuf;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    ExportSpecifier, Expr, ImportDecl, ImportDefaultSpecifier, ImportSpecifier, ModuleDecl,
    ModuleExportName, ModuleItem, Stmt,
};
use mako_core::swc_ecma_utils::quote_str;
use mako_core::swc_ecma_visit::Fold;

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
                // 1. Exclude situations where import source does not need to be replaced
                //   - Don't replace import namesapce
                // FIXME: If the specifiers' length is zero, should replace?
                // FIXME: Consider replace import default
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

                if specifiers.len() == 0 {
                    new_items.push(module_item);
                    continue;
                }

                // 2. resolve dep, and then determine whether it is a barrel file
                let raw_src = import_decl.src.value.to_string();
                let dep = Dependency {
                    source: raw_src.clone(),
                    resolve_type: ResolveType::Import,
                    order: 0,
                    span: None,
                };

                match resolve(&self.path, &dep, &self.context.resolvers, &self.context) {
                    Ok(resolved_resource) => {
                        // Don't replace ignored and external
                        if matches!(
                            resolved_resource,
                            ResolverResource::Ignored | ResolverResource::External(_)
                        ) {
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

                        println!(
                            "\nparsed barrel file:\n    path:{:?}\n    export_map:{:?}\n",
                            path, export_map
                        );

                        // 2. If it's a bucket file, rewrite the source of import statement
                        //   - `import { a } from 'foo';` => `import { a } from 'foo/a';`
                        //   - `import { a, b, bb } from 'foo';` => `import { a } from 'foo/a'; import { b, bb } from 'foo/b';`
                        // src_specifiers_map: [ ('foo/a', [a]), ('foo/b', [b, bb]) ]
                        let mut src_specifiers_map: Vec<(String, Vec<ImportSpecifier>)> = vec![];
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
                            if let Some(export) = export_map
                                .iter()
                                .find(|export| export.0 == imported && export.2 == "default")
                            {
                                // default specifier: `export { default as Button } from 'button';`

                                let new_src = PathBuf::from(&path)
                                    .parent()
                                    .unwrap()
                                    .join(&export.1)
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
                                export_map.iter().find(|export| export.2 == imported)
                            {
                                // named specifier: `export { a } from 'a';`
                                // 'foo/a'
                                let new_src = PathBuf::from(&path)
                                    .parent()
                                    .unwrap()
                                    .join(&export.1)
                                    .to_string_lossy()
                                    .to_string();

                                match src_specifiers_map
                                    .iter_mut()
                                    .find(|(src, _)| src == &new_src)
                                {
                                    Some(map) => map
                                        .1
                                        .push(ImportSpecifier::Named(specifier.clone().clone())),
                                    None => {
                                        src_specifiers_map.push((
                                            new_src,
                                            vec![ImportSpecifier::Named(specifier.clone().clone())],
                                        ));
                                    }
                                }
                            } else {
                                // If the import specifier is not exported from the barrel file, keep the import statement here first
                                match src_specifiers_map
                                    .iter_mut()
                                    .find(|(src, _)| src == &raw_src)
                                {
                                    Some(map) => map
                                        .1
                                        .push(ImportSpecifier::Named(specifier.clone().clone())),
                                    None => {
                                        src_specifiers_map.push((
                                            raw_src.clone(),
                                            vec![ImportSpecifier::Named(specifier.clone().clone())],
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

fn parse_barrel_file(
    path: &str,
    context: &Arc<Context>,
) -> Result<(bool, Vec<(String, String, String)>)> {
    let request = parse_path(path)?;
    let content = load(&request, false, context)?;
    let ast = parse(&content, &request, context)?;
    let ast = ast.as_script();

    // A barrel file to be a file that only exports from other modules.
    // Besides that, lit expressions are allowed as well ("use client", etc.).
    let mut is_barrel = true;
    // Imported meta information. import { a, b as bb } from './foo'; => [(a, './foo', a), (bb, './foo', b)]
    // let mut import_map = vec![];
    // Exportd meta information. export { a, b as bb } from './foo'; => [(a, './foo', a), (bb, './foo', b)]
    let mut export_map = vec![];

    for module_item in &ast.body {
        match module_item {
            ModuleItem::ModuleDecl(module_decl) => {
                match module_decl {
                    // import
                    ModuleDecl::Import(_) => {
                        // Yes
                    }
                    // export named
                    ModuleDecl::ExportNamed(export_named) => {
                        for specifier in &export_named.specifiers {
                            match specifier {
                                // `export { foo } from 'foo';` / `export { foo as bar } from 'foo';`
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
                                        export_map.push((
                                            name_str.clone(),
                                            src.value.to_string(),
                                            orig_str.clone(),
                                        ));
                                    } else {
                                        // FIXME break 需要跳出外层循环
                                        is_barrel = false;
                                        break;
                                    }
                                }
                                // `export * as foo from 'foo';`
                                ExportSpecifier::Namespace(_) => {}
                                // export v from 'mod';
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
                    Expr::Lit(_) => {
                        // Yes
                    }
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

    Ok((is_barrel, export_map))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
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

    fn context(input: &PathBuf) -> Arc<Context> {
        let root = input.parent().unwrap().to_path_buf();
        let config = Config::new(&root, None, None).unwrap();
        let compiler = Compiler::new(config, root.clone(), Default::default()).unwrap();
        compiler.context
    }
}
