use mako_core::collections::HashSet;
use mako_core::swc_ecma_ast::{Decl, Ident};
use mako_core::swc_ecma_visit::{Visit, VisitWith};

use crate::tree_shaking::defined_ident_collector::DefinedIdentCollector;

/**
 * 收集所有使用到的标识符，由此可以分析出声明语句的依赖关系
 */
impl UsedIdentCollector {
    pub fn new() -> Self {
        Self {
            used_ident: HashSet::default(),
            defined_ident: HashSet::default(),
        }
    }
}

impl Visit for UsedIdentCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        let id = ident.to_string();
        // 过滤掉当前 scope 下自己定义的变量
        if !self.defined_ident.contains(&id) {
            self.used_ident.insert(id);
        }
    }

    fn visit_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Class(decl) => {
                self.defined_ident.insert(decl.ident.to_string());
            }
            Decl::Fn(decl) => {
                self.defined_ident.insert(decl.ident.to_string());
            }
            Decl::Var(decl) => {
                for decl in &decl.decls {
                    let mut defined_ident_collector = DefinedIdentCollector::new();
                    decl.name.visit_with(&mut defined_ident_collector);
                    self.used_ident.extend(defined_ident_collector.used_ident);
                    self.defined_ident
                        .extend(defined_ident_collector.defined_ident);
                }
            }
            _ => {}
        }
        decl.visit_children_with(self);
    }
}

pub struct UsedIdentCollector {
    pub used_ident: HashSet<String>,
    defined_ident: HashSet<String>,
}
