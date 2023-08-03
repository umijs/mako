use swc_atoms::js_word;
use swc_common::collections::AHashSet;
use swc_ecma_ast::{
    BreakStmt, CatchClause, ClassExpr, ClassMethod, Constructor, ContinueStmt, Expr, Function, Id,
    Ident, LabeledStmt, MemberExpr, MemberProp, Module, Pat, PropName, SuperProp,
};
use swc_ecma_utils::collect_decls;
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct DetectUnDeclGlobals {
    pub decls: AHashSet<Id>,
    idents: AHashSet<Id>,

    in_fun_decl: bool,
    is_pat_decl: bool,
    is_ignoring: bool,
}

impl DetectUnDeclGlobals {
    pub fn new() -> Self {
        Self {
            decls: Default::default(),
            idents: Default::default(),
            in_fun_decl: false,
            is_pat_decl: false,
            is_ignoring: false,
        }
    }

    pub fn undels(&self) -> Vec<Id> {
        self.idents
            .difference(&self.decls)
            .cloned()
            .collect::<Vec<Id>>()
    }
}

impl VisitMut for DetectUnDeclGlobals {
    fn visit_mut_module(&mut self, module: &mut Module) {
        self.decls = collect_decls(module);
        module.visit_mut_children_with(self);
    }

    fn visit_mut_ident(&mut self, ident: &mut Ident) {
        let id: Id = Id::from(ident.clone());

        if self.in_fun_decl && ident.sym == js_word!("arguments") {
            return;
        }

        if self.is_ignoring {
            return;
        }

        self.idents.insert(id);
    }

    fn visit_mut_constructor(&mut self, c: &mut Constructor) {
        let last = self.in_fun_decl;
        self.in_fun_decl = true;
        c.visit_mut_children_with(self);
        self.in_fun_decl = last;
    }

    fn visit_mut_function(&mut self, fun_decl: &mut Function) {
        let last = self.in_fun_decl;
        self.in_fun_decl = true;
        fun_decl.visit_mut_children_with(self);
        self.in_fun_decl = last;
    }

    fn visit_mut_class_expr(&mut self, class: &mut ClassExpr) {
        class.class.visit_mut_children_with(self);
    }

    fn visit_mut_super_prop(&mut self, super_prop: &mut SuperProp) {
        if let SuperProp::Computed(_) = super_prop {
            super_prop.visit_mut_children_with(self);
        }
    }

    fn visit_mut_member_expr(&mut self, expr: &mut MemberExpr) {
        if let Expr::Ident(ref id) = *expr.obj {
            self.idents.insert(id.clone().into());
        } else {
            expr.obj.visit_mut_with(self);
        }

        expr.prop.visit_mut_with(self);
    }

    fn visit_mut_member_prop(&mut self, prop: &mut MemberProp) {
        if let MemberProp::Computed(_) = prop {
            prop.visit_mut_children_with(self);
        }
    }

    fn visit_mut_pat(&mut self, node: &mut Pat) {
        node.visit_mut_children_with(self);

        if self.is_pat_decl {
            if let Pat::Ident(i) = node {
                self.decls.insert(Id::from(i.id.clone()));
            }
        }
    }

    fn visit_mut_prop_name(&mut self, node: &mut PropName) {
        if let PropName::Computed(_) = node {
            node.visit_mut_children_with(self);
        }
    }

    fn visit_mut_class_method(&mut self, node: &mut ClassMethod) {
        let last = self.is_ignoring;

        if let PropName::Computed(_) = node.key {
            self.is_ignoring = false
        } else {
            self.is_ignoring = true;
        }

        node.key.visit_mut_with(self);
        self.is_ignoring = last;

        node.function.visit_mut_with(self);
    }

    fn visit_mut_catch_clause(&mut self, node: &mut CatchClause) {
        let last = self.is_pat_decl;
        self.is_pat_decl = true;
        node.param.visit_mut_with(self);
        self.is_pat_decl = last;

        node.body.visit_mut_with(self);
    }

    fn visit_mut_labeled_stmt(&mut self, node: &mut LabeledStmt) {
        // ignore node.label
        node.body.visit_mut_with(self);
    }

    fn visit_mut_break_stmt(&mut self, _: &mut BreakStmt) {
        // ignore node.label
    }

    fn visit_mut_continue_stmt(&mut self, _: &mut ContinueStmt) {
        // ignore node.label
    }
}

#[cfg(test)]
#[rustfmt::skip::macros(test)]
#[rustfmt::skip::macros(test_ignore)]
mod tests {
    use std::collections::HashSet;
    use std::env;
    use std::path::PathBuf;
    use std::sync::Arc;

    use swc_common::{Mark, GLOBALS};
    use swc_ecma_transforms::resolver;
    use swc_ecma_visit::VisitMutWith;

    use crate::ast::build_js_ast;
    use crate::compiler::Context;
    use crate::transform_ident::DetectUnDeclGlobals;

    macro_rules! test {
        ($name:ident,$expr:expr) => {
            #[test]
            fn $name() {
                let out_dir: PathBuf = env::var("CARGO_MANIFEST_DIR").unwrap().into();
                let fixture_root = out_dir.join("test/fixtures/transform-ident");

                let f = fixture_root.join(concat!(stringify!($name), ".js"));

                let f = std::fs::read_to_string(f).unwrap();
                let mut expect = $expr.clone();
                expect.sort();

                assert_eq!(detect_un_decl_globals(&f), expect);
            }
        };
    }

    test!(argument,                            none());
    test!(arrow_functions,                     vec!["z", "b", "c", "arguments"]);
    test!(assign_implicit,                     vec!["bar"]);
    test!(catch_pattern,                       none());
    test!(catch_without_error,                 none());
    test!(class,                               vec!["G", "OtherClass_", "SuperClass"]);
    test!(class_field_definition_this,         none());
    test!(class_expression,                    none());
    test!(default_argument,                    vec!["c", "h", "j", "k",]);
    test!(destructuring,                       vec!["g"]);
    test!(detect,                              vec!["w", "foo", "process", "console", "AAA", "BBB", "CCC", "xyz", "ZZZ", "BLARG", "RAWR"]);
    test!(export,                              vec!["baz"]);
    test!(export_default_anonymous_class,      none());
    test!(export_default_anonymous_function,   none());
    test!(import,                              vec!["whatever"]);
    test!(labels,                              none());
    test!(multiple_exports,                    vec!["bar", "exports"]);
    test!(names_in_object_prototype,           vec!["__proto__", "constructor", "hasOwnProperty"]);
    test!(obj,                                 vec!["bar", "module"]);
    test!(properties,                          vec!["qualified_g", "simple_g", "uglier", "ugly"]);
    test!(rest_argument,                       none());
    test!(return_hash,                         none());
    test!(right_hand,                          vec!["exports", "__dirname", "__filename"]);
    test!(switch_statement,                    vec!["a"]);
    test!(this,                                none());
    test!(try_catch,                           none());

    fn detect_un_decl_globals(code: &str) -> Vec<String> {
        let context: Arc<Context> = Arc::new(Default::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut ast = build_js_ast("test.js", code, &context).unwrap();
            let mut detector = DetectUnDeclGlobals::new();

            ast.ast
                .visit_mut_with(&mut resolver(Mark::new(), Mark::new(), false));
            ast.ast.visit_mut_with(&mut detector);

            let mut globals = detector
                .undels()
                .iter()
                .map(|id| id.0.to_string())
                .collect::<HashSet<String>>()
                .iter()
                .cloned()
                .collect::<Vec<String>>();
            globals.sort();

            globals
        })
    }

    fn none() -> Vec<&'static str> {
        Vec::new()
    }
}
