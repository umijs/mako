use std::collections::{HashMap, HashSet};

use swc_core::ecma::ast::Lit::Str;
use swc_core::ecma::ast::{BlockStmt, CallExpr, Callee, ExprOrSpread, Stmt};
use swc_core::ecma::visit::{Visit, VisitWith};

pub struct InteropProbe<'a> {
    pub probed: HashSet<String>,
    pub max_deep: u32,
    current_block_level: u32,
    helpers_map: &'a HashMap<String, String>,
    quick_exit: bool,
}

impl<'a> InteropProbe<'a> {
    pub fn new(map: &'a HashMap<String, String>, max_deep: u32) -> Self {
        InteropProbe {
            probed: HashSet::new(),
            current_block_level: 0,
            max_deep,
            helpers_map: map,
            quick_exit: false,
        }
    }

    fn helper(&self, arg: Option<&ExprOrSpread>) -> Option<&String> {
        arg.and_then(|a| a.expr.as_lit()).and_then(|lit| {
            if let Str(str) = lit {
                let src = str.value.to_string();
                self.helpers_map.get(&src)
            } else {
                None
            }
        })
    }

    fn is_mako_require(&self, callee: &Callee) -> bool {
        if let Some(id) = callee.as_expr()
            && let Some(id) = id.as_ident()
        {
            id.sym.eq("__mako_require__")
        } else {
            false
        }
    }
}

impl Visit for InteropProbe<'_> {
    fn visit_block_stmt(&mut self, n: &BlockStmt) {
        let old = self.current_block_level;

        if old >= self.max_deep || self.quick_exit {
            return;
        }
        self.current_block_level = old + 1;
        n.visit_children_with(self);
        self.current_block_level = old;
    }

    fn visit_call_expr(&mut self, call_expr: &CallExpr) {
        if self.is_mako_require(&call_expr.callee)
            && let Some(helper) = self.helper(call_expr.args.first())
        {
            self.probed.insert(helper.to_string());
            self.quick_exit = self.probed.len() >= 2;
        } else {
            call_expr.visit_children_with(self);
        }
    }

    fn visit_stmt(&mut self, n: &Stmt) {
        if self.quick_exit {
            return;
        }
        n.visit_children_with(self);
    }
}
