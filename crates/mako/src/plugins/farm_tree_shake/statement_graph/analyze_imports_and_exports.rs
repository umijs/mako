use std::collections::{HashMap, HashSet};

use mako_core::swc_common::{Span, SyntaxContext, DUMMY_SP};
use mako_core::swc_ecma_ast;
use mako_core::swc_ecma_ast::{ModuleExportName, ModuleItem};
use mako_core::swc_ecma_visit::VisitWith;

use super::defined_idents_collector::DefinedIdentsCollector;
use super::side_effects_detector::SideEffectsDetector;
use super::used_idents_collector::{self, UsedIdentsCollector};
use super::{ExportInfo, ExportSpecifierInfo, ImportInfo, ImportSpecifierInfo, StatementId};

#[derive(Debug)]
pub struct StatementInfo {
    pub import_info: Option<ImportInfo>,
    pub export_info: Option<ExportInfo>,
    pub defined_idents: HashSet<String>,
    pub used_idents: HashSet<String>,
    pub defined_idents_map: HashMap<String, HashSet<String>>,
    pub is_self_executed: bool,
    pub has_side_effects: bool,
    pub span: Span,
}

pub fn analyze_imports_and_exports(
    id: &StatementId,
    stmt: &ModuleItem,
    used_defined_idents: Option<HashSet<String>>,
    unresolved_ctxt: SyntaxContext,
) -> StatementInfo {
    mako_core::mako_profile_function!();

    let mut defined_idents = HashSet::new();
    let mut used_idents = HashSet::new();
    let mut defined_idents_map = HashMap::new();

    let mut imports = None;
    let mut exports = None;

    let mut is_self_executed = false;
    let mut span = DUMMY_SP;

    let mut analyze_and_insert_used_idents =
        |stmt: &dyn VisitWith<UsedIdentsCollector>, ident: Option<String>| {
            // skip if used_defined_idents is not None as it is only uses the imports and exports for now
            if used_defined_idents.is_some() {
                return;
            }

            let mut used_idents_collector = used_idents_collector::UsedIdentsCollector::new();
            stmt.visit_with(&mut used_idents_collector);

            if let Some(ident) = ident {
                defined_idents_map.insert(ident, used_idents_collector.used_idents.clone());
            }

            used_idents.extend(used_idents_collector.used_idents);
        };

    let is_ident_used = |ident: &String| {
        if let Some(used_defined_idents) = &used_defined_idents {
            return used_defined_idents.contains(ident);
        }

        true
    };

    match stmt {
        ModuleItem::ModuleDecl(module_decl) => match module_decl {
            swc_ecma_ast::ModuleDecl::Import(import_decl) => {
                let source = import_decl.src.value.to_string();
                let mut specifiers = vec![];

                for specifier in &import_decl.specifiers {
                    match specifier {
                        swc_ecma_ast::ImportSpecifier::Namespace(ns) => {
                            if !is_ident_used(&ns.local.to_string()) {
                                continue;
                            }

                            specifiers.push(ImportSpecifierInfo::Namespace(ns.local.to_string()));
                            defined_idents.insert(ns.local.to_string());
                        }
                        swc_ecma_ast::ImportSpecifier::Named(named) => {
                            if !is_ident_used(&named.local.to_string()) {
                                continue;
                            }

                            specifiers.push(ImportSpecifierInfo::Named {
                                local: named.local.to_string(),
                                imported: named.imported.as_ref().map(|i| match i {
                                    ModuleExportName::Ident(i) => i.to_string(),
                                    _ => panic!(
                                        "non-ident imported is not supported when tree shaking"
                                    ),
                                }),
                            });
                            defined_idents.insert(named.local.to_string());
                        }
                        swc_ecma_ast::ImportSpecifier::Default(default) => {
                            if !is_ident_used(&default.local.to_string()) {
                                continue;
                            }

                            specifiers
                                .push(ImportSpecifierInfo::Default(default.local.to_string()));
                            defined_idents.insert(default.local.to_string());
                        }
                    }
                }

                if specifiers.is_empty() {
                    // TODO: import "x" may not be a side effect statement
                    is_self_executed = true;
                }

                span = import_decl.span;

                imports = Some(ImportInfo {
                    source,
                    specifiers,
                    stmt_id: *id,
                });
            }
            swc_ecma_ast::ModuleDecl::ExportAll(export_all) => {
                span = export_all.span;
                exports = Some(ExportInfo {
                    source: Some(export_all.src.value.to_string()),
                    specifiers: vec![ExportSpecifierInfo::All(vec![])],
                    stmt_id: *id,
                })
            }
            swc_ecma_ast::ModuleDecl::ExportDecl(export_decl) => {
                span = export_decl.span;

                match &export_decl.decl {
          swc_ecma_ast::Decl::Class(class_decl) => {
            exports = Some(ExportInfo {
              source: None,
              specifiers: vec![ExportSpecifierInfo::Named { local: class_decl.ident.to_string(), exported: None }],
              stmt_id: *id,
            });
            defined_idents.insert(class_decl.ident.to_string());
            analyze_and_insert_used_idents(&class_decl.class, Some(class_decl.ident.to_string()));
          },
          swc_ecma_ast::Decl::Fn(fn_decl) => {
            exports = Some(ExportInfo {
              source: None,
              specifiers: vec![ExportSpecifierInfo::Named { local: fn_decl.ident.to_string(), exported: None }],
              stmt_id: *id,
            });
            defined_idents.insert(fn_decl.ident.to_string());
            analyze_and_insert_used_idents(&fn_decl.function, Some(fn_decl.ident.to_string()));
          },
          swc_ecma_ast::Decl::Var(var_decl) => {
            let mut specifiers = vec![];

            for v_decl in &var_decl.decls {

              let mut defined_idents_collector = DefinedIdentsCollector::new();
              v_decl.name.visit_with(&mut defined_idents_collector);
              let mut used_idents_collector = UsedIdentsCollector::new();

              if let Some(init) = &v_decl.init {
                init.visit_with(&mut used_idents_collector);
              }

              let mut local_used_idents = HashSet::new();
              local_used_idents.extend(used_idents_collector.used_idents);
              local_used_idents.extend(defined_idents_collector.used_idents);
              used_idents.extend(local_used_idents.clone());

              for defined_ident in defined_idents_collector.defined_idents {
                if !is_ident_used(&defined_ident.to_string()) {
                  continue;
                }

                specifiers.push(ExportSpecifierInfo::Named { local: defined_ident.to_string(), exported: None });
                defined_idents.insert(defined_ident.clone());
                defined_idents_map.insert(defined_ident.clone(), local_used_idents.clone());
              }
            }

            exports = Some(ExportInfo {
              source: None,
              specifiers,
              stmt_id: *id,
            });
          },
          _ => unreachable!("export_decl.decl should not be anything other than a class, function, or variable declaration"),
        }
            }
            swc_ecma_ast::ModuleDecl::ExportDefaultDecl(export_default_decl) => {
                span = export_default_decl.span;

                exports = Some(ExportInfo {
                    source: None,
                    specifiers: vec![ExportSpecifierInfo::Default],
                    stmt_id: *id,
                });
                match &export_default_decl.decl {
                    swc_ecma_ast::DefaultDecl::Class(class_expr) => {
                        if let Some(ident) = &class_expr.ident {
                            defined_idents.insert(ident.to_string());
                        }
                        analyze_and_insert_used_idents(
                            &class_expr.class,
                            class_expr.ident.as_ref().map(|i| i.to_string()),
                        );
                    }
                    swc_ecma_ast::DefaultDecl::Fn(fn_decl) => {
                        if let Some(ident) = &fn_decl.ident {
                            defined_idents.insert(ident.to_string());
                        }
                        analyze_and_insert_used_idents(
                            &fn_decl.function,
                            fn_decl.ident.as_ref().map(|i| i.to_string()),
                        );
                    }
                    _ => unreachable!(
            "export_default_decl.decl should not be anything other than a class, function"
          ),
                }
            }
            swc_ecma_ast::ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                span = export_default_expr.span;

                exports = Some(ExportInfo {
                    source: None,
                    specifiers: vec![ExportSpecifierInfo::Default],
                    stmt_id: *id,
                });
                analyze_and_insert_used_idents(&export_default_expr.expr, None);
            }
            swc_ecma_ast::ModuleDecl::ExportNamed(export_named) => {
                span = export_named.span;

                let mut specifiers = vec![];

                for specifier in &export_named.specifiers {
                    match specifier {
                        swc_ecma_ast::ExportSpecifier::Named(named) => {
                            let local = match &named.orig {
                                ModuleExportName::Ident(i) => i.clone(),
                                ModuleExportName::Str(_) => {
                                    unimplemented!("exporting a string is not supported")
                                }
                            };

                            if !is_ident_used(&local.to_string()) {
                                continue;
                            }

                            if export_named.src.is_none() {
                                used_idents.insert(local.to_string());
                                defined_idents_map
                                    .insert(local.to_string(), [local.to_string()].into());
                            }

                            specifiers.push(ExportSpecifierInfo::Named {
                                local: local.to_string(),
                                exported: named.exported.as_ref().map(|i| match i {
                                    ModuleExportName::Ident(i) => i.to_string(),
                                    _ => panic!(
                                        "non-ident exported is not supported when tree shaking"
                                    ),
                                }),
                            });
                        }
                        swc_ecma_ast::ExportSpecifier::Default(_) => {
                            unreachable!("ExportSpecifier::Default is not valid esm syntax")
                        }
                        swc_ecma_ast::ExportSpecifier::Namespace(ns) => {
                            let ident = match &ns.name {
                                ModuleExportName::Ident(ident) => ident.to_string(),
                                ModuleExportName::Str(_) => {
                                    unreachable!("exporting a string is not supported")
                                }
                            };

                            specifiers.push(ExportSpecifierInfo::Namespace(ident));
                        }
                    }
                }

                exports = Some(ExportInfo {
                    source: export_named.src.as_ref().map(|s| s.value.to_string()),
                    specifiers,
                    stmt_id: *id,
                });
            }
            _ => {}
        },
        ModuleItem::Stmt(stmt) => {
            match stmt {
                swc_ecma_ast::Stmt::Block(block) => {
                    span = block.span;
                    analyze_and_insert_used_idents(block, None);

                }
                swc_ecma_ast::Stmt::Empty(_) => {}
                swc_ecma_ast::Stmt::Debugger(_) => {}
                swc_ecma_ast::Stmt::With(with) => {
                    span = with.span;
                    analyze_and_insert_used_idents(with, None);
                }
                swc_ecma_ast::Stmt::Return(_) => {
                    unreachable!("return statement should not be present in a module root")
                }
                swc_ecma_ast::Stmt::Labeled(label) => {
                    span = label.span;

                    analyze_and_insert_used_idents(label, None);

                }
                swc_ecma_ast::Stmt::Break(_) => {
                    unreachable!("break statement should not be present in a module root")
                }
                swc_ecma_ast::Stmt::Continue(_) => {
                    unreachable!("continue statement should not be present in a module root")
                }
                swc_ecma_ast::Stmt::If(if_stmt) => {
                    span = if_stmt.span;

                    analyze_and_insert_used_idents(if_stmt, None);

                }
                swc_ecma_ast::Stmt::Switch(switch_stmt) => {
                    span = switch_stmt.span;

                    analyze_and_insert_used_idents(switch_stmt, None)
                }
                swc_ecma_ast::Stmt::Throw(throw) => {
                    span = throw.span;

                    analyze_and_insert_used_idents(throw, None)
                }
                swc_ecma_ast::Stmt::Try(try_stmt) => {
                    span = try_stmt.span;

                    analyze_and_insert_used_idents(try_stmt, None)


                }
                swc_ecma_ast::Stmt::While(while_stmt) => {
                    span = while_stmt.span;

                    analyze_and_insert_used_idents(while_stmt, None)


                }
                swc_ecma_ast::Stmt::DoWhile(do_while) => {
                    span = do_while.span;

                    analyze_and_insert_used_idents(do_while, None)
                }
                swc_ecma_ast::Stmt::For(for_stmt) => {
                    span = for_stmt.span;

                    analyze_and_insert_used_idents(for_stmt, None)
                }

                swc_ecma_ast::Stmt::ForIn(for_in) => {
                    span = for_in.span;

                    analyze_and_insert_used_idents(for_in, None)
                }
                swc_ecma_ast::Stmt::ForOf(for_of) => {
                    span = for_of.span;

                    analyze_and_insert_used_idents(for_of, None)
                }
                swc_ecma_ast::Stmt::Decl(decl) => match decl {
                    swc_ecma_ast::Decl::Class(class_decl) => {
                        defined_idents.insert(class_decl.ident.to_string());
                        analyze_and_insert_used_idents(
                            &class_decl.class,
                            Some(class_decl.ident.to_string()),
                        );
                    }
                    swc_ecma_ast::Decl::Fn(fn_decl) => {
                        defined_idents.insert(fn_decl.ident.to_string());
                        analyze_and_insert_used_idents(
                            &fn_decl.function,
                            Some(fn_decl.ident.to_string()),
                        );
                    }
                    swc_ecma_ast::Decl::Var(var_decl) => {
                        for v_decl in &var_decl.decls {
                            let mut defined_idents_collector = DefinedIdentsCollector::new();
                            v_decl.name.visit_with(&mut defined_idents_collector);
                            let mut used_idents_collector = UsedIdentsCollector::new();

                            if let Some(init) = &v_decl.init {
                                init.visit_with(&mut used_idents_collector);
                            }

                            let mut local_used_idents = HashSet::new();
                            local_used_idents.extend(used_idents_collector.used_idents);
                            local_used_idents.extend(defined_idents_collector.used_idents);
                            used_idents.extend(local_used_idents.clone());

                            for defined_ident in defined_idents_collector.defined_idents {
                                defined_idents.insert(defined_ident.clone());
                                defined_idents_map
                                    .insert(defined_ident.clone(), local_used_idents.clone());
                            }

                        }
                    }
                    _ => unreachable!(
                        "decl should not be anything other than a class, function, or variable declaration"
                    ),
                },
                swc_ecma_ast::Stmt::Expr(expr) => {
                    span = expr.span;
                    analyze_and_insert_used_idents(expr, None);
                }
            }

            let mut detector = SideEffectsDetector::new(unresolved_ctxt);
            stmt.visit_with(&mut detector);
            is_self_executed = detector.has_side_effects();
        }
    };

    StatementInfo {
        import_info: imports,
        export_info: exports,
        defined_idents,
        used_idents,
        defined_idents_map,
        is_self_executed,
        has_side_effects: false,
        span,
    }
}
