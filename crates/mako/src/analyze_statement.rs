use std::collections::HashSet;

use swc_ecma_ast::{ModuleExportName, ModuleItem};
use swc_ecma_visit::VisitWith;

use crate::defined_ident_collector::DefinedIdentCollector;
use crate::statement::{
    ExportInfo, ExportSpecifier, ExportStatement, ImportInfo, ImportSpecifier, ImportStatement,
    StatementId, StatementType,
};
use crate::used_ident_collector::UsedIdentCollector;

/**
 * 分析当前传入的 esm ast，返回 StatementType
 */
pub fn analyze_statement(id: StatementId, statement: &ModuleItem) -> StatementType {
    let mut top_level_defined_ident = HashSet::new();
    let mut used_ident = HashSet::new();
    let mut is_self_executed = false;

    let mut analyze_used_ident_from_statement =
        |statement: &dyn VisitWith<UsedIdentCollector>, _ident: Option<String>| {
            let mut used_ident_collector = UsedIdentCollector::new();
            statement.visit_with(&mut used_ident_collector);
            used_ident.extend(used_ident_collector.used_ident);
        };

    match statement {
        ModuleItem::ModuleDecl(module_decl) => {
            match module_decl {
                swc_ecma_ast::ModuleDecl::Import(decl) => {
                    // import xx from "source";
                    let source = decl.src.value.to_string();
                    let mut specifiers = Vec::new();
                    for specifier in &decl.specifiers {
                        match specifier {
                            swc_ecma_ast::ImportSpecifier::Named(named) => {
                                specifiers.push(ImportSpecifier::Named {
                                    local: named.local.to_string(),
                                    imported: named.imported.as_ref().map(|i| match i {
                                        ModuleExportName::Ident(i) => i.to_string(),
                                        _ => panic!(
                                            "non-ident imported is not supported when tree shaking"
                                        ),
                                    }),
                                });
                                top_level_defined_ident.insert(named.local.to_string());
                            }
                            swc_ecma_ast::ImportSpecifier::Default(default) => {
                                specifiers
                                    .push(ImportSpecifier::Default(default.local.to_string()));
                                top_level_defined_ident.insert(default.local.to_string());
                            }
                            swc_ecma_ast::ImportSpecifier::Namespace(namespace) => {
                                specifiers
                                    .push(ImportSpecifier::Namespace(namespace.local.to_string()));
                                top_level_defined_ident.insert(namespace.local.to_string());
                            }
                        }
                    }

                    // import "source";
                    if specifiers.is_empty() {
                        is_self_executed = true;
                    }

                    StatementType::Import(ImportStatement {
                        id,
                        info: ImportInfo {
                            source,
                            specifiers,
                            stmt_id: id,
                        },
                        is_self_executed,
                        defined_ident: top_level_defined_ident,
                    })
                }
                swc_ecma_ast::ModuleDecl::ExportDecl(export_decl) => {
                    match &export_decl.decl {
                        // export class Foo {}
                        swc_ecma_ast::Decl::Class(decl) => {
                            top_level_defined_ident.insert(decl.ident.to_string());
                            analyze_used_ident_from_statement(
                                &decl.class,
                                Some(decl.ident.to_string()),
                            );
                            StatementType::Export(ExportStatement {
                                id,
                                info: ExportInfo {
                                    source: None,
                                    specifiers: vec![ExportSpecifier::Named {
                                        local: decl.ident.to_string(),
                                        exported: None,
                                    }],
                                    stmt_id: id,
                                },
                                defined_ident: top_level_defined_ident,
                                used_ident,
                            })
                        }
                        // export function foo() {}
                        swc_ecma_ast::Decl::Fn(decl) => {
                            top_level_defined_ident.insert(decl.ident.to_string());
                            analyze_used_ident_from_statement(
                                &decl.function,
                                Some(decl.ident.to_string()),
                            );
                            StatementType::Export(ExportStatement {
                                info: ExportInfo {
                                    source: None,
                                    specifiers: vec![ExportSpecifier::Named {
                                        local: decl.ident.to_string(),
                                        exported: None,
                                    }],
                                    stmt_id: id,
                                },
                                defined_ident: top_level_defined_ident,
                                used_ident,
                                id,
                            })
                        }
                        // export const foo = 1;
                        swc_ecma_ast::Decl::Var(decl) => {
                            let mut specifiers = vec![];

                            for decl in &decl.decls {
                                // var [a,1]=[1,2]; 左边的模式中需要继续需要进入分析
                                let (defined_ident_collector, used_ident_collector) =
                                    collect_ident_for_var_decl(decl);

                                used_ident.extend(defined_ident_collector.used_ident);
                                used_ident.extend(used_ident_collector.used_ident);

                                for ident in defined_ident_collector.defined_ident {
                                    specifiers.push(ExportSpecifier::Named {
                                        local: ident.to_string(),
                                        exported: None,
                                    });
                                    top_level_defined_ident.insert(ident.clone());
                                }
                            }
                            StatementType::Export(ExportStatement {
                                info: ExportInfo {
                                    source: None,
                                    specifiers,
                                    stmt_id: id,
                                },
                                defined_ident: top_level_defined_ident,
                                used_ident,
                                id,
                            })
                        }
                        // 下面这些TS相关的一般是提前转换过的，不会出现在这里
                        _decl => {
                            unreachable!("export Ts decl not supported");
                        }
                    }
                }
                swc_ecma_ast::ModuleDecl::ExportNamed(decl) => {
                    let mut specifiers = vec![];
                    for specifier in &decl.specifiers {
                        match specifier {
                            // export * as foo from '..';
                            swc_ecma_ast::ExportSpecifier::Namespace(specifier) => {
                                let ident = match &specifier.name {
                                    ModuleExportName::Ident(i) => i.to_string(),
                                    ModuleExportName::Str(_) => {
                                        unreachable!("str as ident is not supported")
                                    }
                                };
                                specifiers.push(ExportSpecifier::Namespace(ident));
                            }
                            swc_ecma_ast::ExportSpecifier::Default(_) => {
                                unreachable!("export default not supported in ExportNamed")
                            }
                            swc_ecma_ast::ExportSpecifier::Named(specifier) => {
                                // `foo` in `export { foo as bar }`
                                let local = match &specifier.orig {
                                    ModuleExportName::Ident(i) => i.clone(),
                                    ModuleExportName::Str(_) => {
                                        unreachable!("str as ident is not supported")
                                    }
                                };

                                // export 没有 from 的情况，标记为定义
                                if decl.src.is_none() {
                                    used_ident.insert(local.to_string());
                                }

                                specifiers.push(ExportSpecifier::Named {
                                    local: local.to_string(),
                                    exported: specifier.exported.as_ref().map(|i| match i {
                                        ModuleExportName::Ident(i) => i.to_string(),
                                        ModuleExportName::Str(_) => {
                                            unreachable!("str as ident is not supported")
                                        }
                                    }),
                                });
                            }
                        }
                    }
                    return StatementType::Export(ExportStatement {
                        info: ExportInfo {
                            source: decl.src.as_ref().map(|s| s.value.to_string()),
                            specifiers,
                            stmt_id: id,
                        },
                        defined_ident: top_level_defined_ident,
                        used_ident,
                        id,
                    });
                }
                swc_ecma_ast::ModuleDecl::ExportDefaultDecl(decl) => {
                    match &decl.decl {
                        swc_ecma_ast::DefaultDecl::Class(decl) => {
                            analyze_used_ident_from_statement(&decl.class, None);
                            if let Some(ident) = &decl.ident {
                                top_level_defined_ident.insert(ident.to_string());
                            }
                        }
                        swc_ecma_ast::DefaultDecl::Fn(decl) => {
                            analyze_used_ident_from_statement(&decl.function, None);
                            if let Some(ident) = &decl.ident {
                                top_level_defined_ident.insert(ident.to_string());
                            }
                        }
                        swc_ecma_ast::DefaultDecl::TsInterfaceDecl(_) => {}
                    }
                    StatementType::Export(ExportStatement {
                        info: ExportInfo {
                            source: None,
                            specifiers: vec![ExportSpecifier::Default],
                            stmt_id: id,
                        },
                        defined_ident: top_level_defined_ident,
                        used_ident,
                        id,
                    })
                }
                swc_ecma_ast::ModuleDecl::ExportDefaultExpr(decl) => {
                    analyze_used_ident_from_statement(&decl.expr, None);
                    StatementType::Export(ExportStatement {
                        info: ExportInfo {
                            source: None,
                            specifiers: vec![ExportSpecifier::Default],
                            stmt_id: id,
                        },
                        defined_ident: top_level_defined_ident,
                        used_ident,
                        id,
                    })
                }
                swc_ecma_ast::ModuleDecl::ExportAll(export_all) => {
                    StatementType::Export(ExportStatement {
                        info: ExportInfo {
                            source: Some(export_all.src.value.to_string()),
                            stmt_id: id,
                            specifiers: vec![ExportSpecifier::All(None)],
                        },
                        defined_ident: top_level_defined_ident,
                        used_ident,
                        id,
                    })
                }
                _ => {
                    unreachable!("export Ts decl not supported");
                }
            }
        }
        ModuleItem::Stmt(statement) => {
            match statement {
                swc_ecma_ast::Stmt::Block(block) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(block, None);
                }
                swc_ecma_ast::Stmt::Empty(_) => {
                    // TODO: implement it
                }
                swc_ecma_ast::Stmt::Debugger(_) => {
                    // TODO: implement it
                }
                swc_ecma_ast::Stmt::With(with) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(with, None);
                }
                swc_ecma_ast::Stmt::Return(_) => {
                    unreachable!("return statement is not supported");
                }
                swc_ecma_ast::Stmt::Labeled(label) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(label, None);
                }
                swc_ecma_ast::Stmt::Break(_) => {
                    unreachable!("break statement is not supported");
                }
                swc_ecma_ast::Stmt::Continue(_) => {
                    unreachable!("continue statement is not supported");
                }
                swc_ecma_ast::Stmt::If(if_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(if_statement, None);
                }
                swc_ecma_ast::Stmt::Switch(switch_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(switch_statement, None);
                }
                swc_ecma_ast::Stmt::Throw(throw_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(throw_statement, None);
                }
                swc_ecma_ast::Stmt::Try(try_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(try_statement, None);
                }
                swc_ecma_ast::Stmt::While(while_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(while_statement, None);
                }
                swc_ecma_ast::Stmt::DoWhile(do_while_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(do_while_statement, None);
                }
                swc_ecma_ast::Stmt::For(for_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(for_statement, None);
                }
                swc_ecma_ast::Stmt::ForIn(for_in_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(for_in_statement, None);
                }
                swc_ecma_ast::Stmt::ForOf(for_of_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(for_of_statement, None);
                }
                swc_ecma_ast::Stmt::Decl(decl) => match decl {
                    swc_ecma_ast::Decl::Class(class_decl) => {
                        top_level_defined_ident.insert(class_decl.ident.to_string());
                        analyze_used_ident_from_statement(
                            &class_decl.class,
                            Some(class_decl.ident.to_string()),
                        );
                    }
                    swc_ecma_ast::Decl::Fn(fn_decl) => {
                        top_level_defined_ident.insert(fn_decl.ident.to_string());
                        analyze_used_ident_from_statement(
                            &fn_decl.function,
                            Some(fn_decl.ident.to_string()),
                        );
                    }
                    swc_ecma_ast::Decl::Var(var_decl) => {
                        for decl in &var_decl.decls {
                            let (defined_ident_collector, used_ident_collector) =
                                collect_ident_for_var_decl(decl);

                            used_ident.extend(defined_ident_collector.used_ident);
                            used_ident.extend(used_ident_collector.used_ident);

                            for ident in defined_ident_collector.defined_ident {
                                top_level_defined_ident.insert(ident.clone());
                            }
                        }
                    }
                    _ => {
                        unreachable!("Only class, function and var declaration is supported");
                    }
                },
                swc_ecma_ast::Stmt::Expr(expr_statement) => {
                    is_self_executed = true;
                    analyze_used_ident_from_statement(expr_statement, None)
                }
            }
            StatementType::Stmt {
                defined_ident: top_level_defined_ident,
                used_ident,
                is_self_executed,
                id,
            }
        }
    }
}

fn collect_ident_for_var_decl(
    decl: &swc_ecma_ast::VarDeclarator,
) -> (DefinedIdentCollector, UsedIdentCollector) {
    let mut defined_ident_collector = DefinedIdentCollector::new();
    decl.name.visit_with(&mut defined_ident_collector);

    let mut used_ident_collector = UsedIdentCollector::new();

    // var a = 1; 右边的初始化表达式为 init
    if let Some(init) = &decl.init {
        // 分析init中使用到的标识符
        init.visit_with(&mut used_ident_collector);
    }
    (defined_ident_collector, used_ident_collector)
}
