use std::collections::{BTreeMap, HashMap, HashSet};

use bitflags::bitflags;
use serde::Serialize;
use swc_core::base::atoms::JsWord;
use swc_core::common::collections::AHashSet;
use swc_core::common::{Mark, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    ClassExpr, DefaultDecl, ExportDefaultDecl, Expr, FnExpr, Id, Ident, KeyValueProp, MemberExpr,
    Module, ModuleDecl, ModuleItem, ObjectLit, Prop, PropOrSpread, VarDeclKind,
};
use swc_core::ecma::utils::{
    collect_decls, collect_decls_with_ctxt, member_expr, quote_ident, quote_str, ExprFactory,
};
use swc_core::ecma::visit::{Visit, VisitWith};

use crate::module::{ImportType, ModuleId, NamedExportType, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::tree_shaking::shake::module_concatenate::ConcatenateConfig;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct EsmDependantFlags: u16 {
        const Default = 1;
        const Named = 1<<2;
        const ExportAll = 1<<3;
        const Namespace = 1<<4; // import * as foo, export * as foo
    }
}
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default, Ord, PartialOrd)]
    pub struct RuntimeFlags: u16 {
        const DefaultInterOp  =1;
        const WildcardInterOp = 1<<2;
        const ExportStartInterOp = 1<<3;
    }
}

macro_rules! require {
    ($src: expr) => {
        quote_ident!("require").as_call(DUMMY_SP, vec![quote_str!(DUMMY_SP, $src).as_arg()])
    };
}

impl EsmDependantFlags {
    pub fn inject_external_export_decl(
        &self,
        src: &str,
        names: &(String, String),
        interop_ident_map: &BTreeMap<RuntimeFlags, String>,
    ) -> Vec<ModuleItem> {
        let mut interopee = require!(src);
        let runtime_helpers: RuntimeFlags = self.into();

        if self.contains(EsmDependantFlags::ExportAll) {
            let ident = interop_ident_map
                .get(&RuntimeFlags::ExportStartInterOp)
                .unwrap()
                .clone();

            interopee = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(
                DUMMY_SP,
                vec![interopee.as_arg(), quote_ident!("exports").as_arg()],
            );
        }

        let cjs_expose_dcl: ModuleItem = interopee
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.0.clone()).into())
            .into();

        if runtime_helpers.contains(RuntimeFlags::WildcardInterOp) {
            let ident = interop_ident_map
                .get(&RuntimeFlags::WildcardInterOp)
                .unwrap()
                .clone();

            let ems_expose_dcl: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_dcl, ems_expose_dcl];
        }

        if runtime_helpers.contains(RuntimeFlags::DefaultInterOp) {
            let interop_ident = interop_ident_map
                .get(&RuntimeFlags::DefaultInterOp)
                .unwrap()
                .clone();

            let esm_expose_stmt: ModuleItem = MemberExpr {
                span: DUMMY_SP,
                obj: quote_ident!(interop_ident).into(),
                prop: quote_ident!("_").into(),
            }
            .as_call(DUMMY_SP, vec![quote_ident!(names.0.clone()).as_arg()])
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

            return vec![cjs_expose_dcl, esm_expose_stmt];
        }

        if names.0.eq(&names.1) {
            return vec![cjs_expose_dcl];
        }

        let ems_expose_stmt: ModuleItem = quote_ident!(names.0.clone())
            .into_var_decl(VarDeclKind::Var, quote_ident!(names.1.clone()).into())
            .into();

        vec![cjs_expose_dcl, ems_expose_stmt]
    }
}

macro_rules! dcl {
    ($name: expr, $init:expr ) => {
        $init
            .into_var_decl(VarDeclKind::Var, quote_ident!($name).into())
            .into()
    };
}

impl RuntimeFlags {
    pub fn need_op(&self) -> bool {
        self.contains(RuntimeFlags::DefaultInterOp) || self.contains(RuntimeFlags::WildcardInterOp)
    }

    pub fn op_ident(&self) -> String {
        match *self {
            RuntimeFlags::DefaultInterOp => "_interop_require_default".to_string(),
            RuntimeFlags::WildcardInterOp => "_interop_require_wildcard".to_string(),
            RuntimeFlags::ExportStartInterOp => "_export_star".to_string(),
            _ => {
                unreachable!();
            }
        }
    }

    pub fn dcl_with(&self, ident: &str) -> ModuleItem {
        match *self {
            RuntimeFlags::DefaultInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_interop_require_default"))
            }
            RuntimeFlags::WildcardInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_interop_require_wildcard"))
            }
            RuntimeFlags::ExportStartInterOp => {
                dcl!(ident, require!("@swc/helpers/_/_export_star"))
            }
            _ => {
                unreachable!();
            }
        }
    }
}

impl From<&EsmDependantFlags> for RuntimeFlags {
    fn from(value: &EsmDependantFlags) -> Self {
        let mut rt_flags = RuntimeFlags::empty();
        if value.contains(EsmDependantFlags::Default) {
            rt_flags.insert(RuntimeFlags::DefaultInterOp);
        }

        if value.contains(EsmDependantFlags::Namespace)
            || value.contains(EsmDependantFlags::Named | EsmDependantFlags::Default)
        {
            rt_flags.remove(RuntimeFlags::DefaultInterOp);
            rt_flags.insert(RuntimeFlags::WildcardInterOp);
        }

        if value.contains(EsmDependantFlags::ExportAll) {
            rt_flags.insert(RuntimeFlags::ExportStartInterOp)
        }

        rt_flags
    }
}

impl From<EsmDependantFlags> for RuntimeFlags {
    fn from(value: EsmDependantFlags) -> Self {
        (&value).into()
    }
}

impl From<&ImportType> for EsmDependantFlags {
    fn from(value: &ImportType) -> Self {
        let mut interops = EsmDependantFlags::empty();
        value.iter().for_each(|x| match x {
            ImportType::Default => {
                interops.insert(EsmDependantFlags::Default);
            }
            ImportType::Namespace => {
                interops.insert(EsmDependantFlags::Namespace);
            }
            ImportType::Named => {
                interops.insert(EsmDependantFlags::Named);
            }
            _ => {}
        });
        interops
    }
}

impl From<&NamedExportType> for EsmDependantFlags {
    fn from(value: &NamedExportType) -> Self {
        let mut res = Self::empty();

        value.iter().for_each(|x| match x {
            NamedExportType::Default => {
                res.insert(EsmDependantFlags::Default);
            }
            NamedExportType::Named => {
                res.insert(EsmDependantFlags::Named);
            }
            NamedExportType::Namespace => {
                res.insert(EsmDependantFlags::Namespace);
            }
            _ => {}
        });
        res
    }
}

impl From<&ResolveType> for EsmDependantFlags {
    fn from(value: &ResolveType) -> Self {
        match value {
            ResolveType::Import(import_type) => import_type.into(),
            ResolveType::ExportNamed(named_export_type) => named_export_type.into(),
            ResolveType::ExportAll => EsmDependantFlags::ExportAll,
            ResolveType::Require => EsmDependantFlags::empty(),
            ResolveType::DynamicImport => EsmDependantFlags::empty(),
            ResolveType::Css => EsmDependantFlags::empty(),
            ResolveType::Worker => EsmDependantFlags::empty(),
        }
    }
}

pub type ModuleRef = (Ident, Option<JsWord>);
pub type ImportModuleRefMap = HashMap<Id, ModuleRef>;

pub fn module_ref_to_expr(module_ref: &ModuleRef) -> Expr {
    match module_ref {
        (id, None) => quote_ident!(id.sym.clone()).into(),
        (id, Some(field)) => MemberExpr {
            span: DUMMY_SP,
            obj: quote_ident!(id.sym.clone()).into(),
            prop: quote_ident!(field.clone()).into(),
        }
        .into(),
    }
}

pub fn all_referenced_variables(module_ref_map: &ImportModuleRefMap) -> HashSet<String> {
    let mut vars = HashSet::new();
    module_ref_map.values().for_each(|(id, _)| {
        vars.insert(id.sym.to_string());
    });
    vars
}

#[derive(Debug, Default)]
pub struct ConcatenateContext {
    pub modules_exports_map: HashMap<ModuleId, HashMap<String, ModuleRef>>,
    pub top_level_vars: HashSet<String>,
    pub external_module_namespace: HashMap<ModuleId, (String, String)>,
    pub interop_idents: BTreeMap<RuntimeFlags, String>,
    pub interop_module_items: Vec<ModuleItem>,
}

impl ConcatenateContext {
    pub fn init(config: &ConcatenateConfig, module_graph: &ModuleGraph) -> Self {
        let mut all_used_globals = HashSet::new();
        config.inners.iter().for_each(|inner| {
            let module = module_graph.get_module(inner).unwrap();
            let ast = &module.as_script().unwrap();
            all_used_globals.extend(ConcatenateContext::global_vars(
                &ast.ast,
                ast.unresolved_mark,
            ));
        });

        let mut context = Self {
            top_level_vars: all_used_globals,
            ..Default::default()
        };
        context.setup_runtime_interops(config.merged_runtime_flags());

        context
    }

    pub fn top_level_vars(ast: &Module, top_level_mark: Mark) -> HashSet<String> {
        let mut top_level_vars = HashSet::new();
        top_level_vars.extend(
            collect_decls_with_ctxt(ast, SyntaxContext::empty().apply_mark(top_level_mark))
                .iter()
                .map(|id: &Id| id.0.to_string()),
        );

        top_level_vars.extend(
            collect_export_default_decl_ident(ast)
                .iter()
                .map(|id| id.0.to_string()),
        );

        top_level_vars
    }

    pub fn all_decls(ast: &Module) -> AHashSet<Id> {
        let mut decls = collect_decls(ast);

        decls.extend(collect_export_default_decl_ident(ast).drain());
        decls.extend(collect_named_fn_expr_ident(ast));

        decls
    }

    pub fn global_vars(ast: &Module, unresolved_mark: Mark) -> HashSet<String> {
        let mut globals = HashSet::new();

        let mut collector = GlobalCollect::new(SyntaxContext::empty().apply_mark(unresolved_mark));
        ast.visit_with(&mut collector);
        globals.extend(
            collector
                .refed_globals
                .iter()
                .map(|id: &Id| id.0.to_string()),
        );

        globals
    }

    pub fn add_external_names(&mut self, external_id: &ModuleId, names: (String, String)) {
        self.external_module_namespace
            .insert(external_id.clone(), names);
    }
    pub fn request_safe_var_name(&mut self, base_name: &str) -> String {
        let name = self.get_safe_var_name(base_name);
        self.add_top_level_var(&name);
        name
    }

    pub fn get_safe_var_name(&self, base_name: &str) -> String {
        let mut name = base_name.to_string();

        let mut post_fix = 0;
        while self.top_level_vars.contains(&name) {
            post_fix += 1;
            name = format!("{}_{}", base_name, post_fix);
        }
        name
    }

    pub fn negotiate_safe_var_name(
        &self,
        occupied_names: &HashSet<String>,
        base_name: &str,
    ) -> String {
        let mut name = base_name.to_string();

        let mut post_fix = 0;
        while self.top_level_vars.contains(&name) || occupied_names.contains(&name) {
            post_fix += 1;
            name = format!("{}_{}", base_name, post_fix);
        }

        name
    }

    pub fn external_expose_names(&self, module_id: &ModuleId) -> Option<&(String, String)> {
        self.external_module_namespace.get(module_id)
    }

    pub fn root_exports_stmts(&self, root_module_id: &ModuleId) -> Vec<ModuleItem> {
        if let Some(export_ref_map) = self.modules_exports_map.get(root_module_id) {
            let mut module_items = vec![];

            let ordered_exports: BTreeMap<_, _> = export_ref_map
                .iter()
                .map(|(k, module_ref)| (k.clone(), module_ref_to_expr(module_ref)))
                .collect();

            let key_value_props: Vec<PropOrSpread> = ordered_exports
                .into_iter()
                .map(|(k, ref_expr)| {
                    Prop::KeyValue(KeyValueProp {
                        key: quote_ident!(k).into(),
                        value: ref_expr.into_lazy_fn(vec![]).into(),
                    })
                    .into()
                })
                .collect();

            // __mako_require__.d(exports, __esModule, { value: true });
            let esm_compat = member_expr!(DUMMY_SP, __mako_require__.d)
                .as_call(
                    DUMMY_SP,
                    vec![
                        quote_ident!("exports").as_arg(),
                        quote_str!("__esModule").as_arg(),
                        ObjectLit {
                            props: [Prop::KeyValue(KeyValueProp {
                                key: quote_ident!("value").into(),
                                value: true.into(),
                            })
                            .into()]
                            .into(),
                            span: DUMMY_SP,
                        }
                        .as_arg(),
                    ],
                )
                .into_stmt()
                .into();

            module_items.push(esm_compat);

            if !key_value_props.is_empty() {
                // __mako_require__.e(exports, { exported: function(){ return v}, ... });
                let export_stmt = member_expr!(DUMMY_SP, __mako_require__.e)
                    .as_call(
                        DUMMY_SP,
                        vec![
                            quote_ident!("exports").as_arg(),
                            ObjectLit {
                                props: key_value_props,
                                span: DUMMY_SP,
                            }
                            .as_arg(),
                        ],
                    )
                    .into_stmt()
                    .into();
                module_items.push(export_stmt);
            }
            module_items
        } else {
            unreachable!("root exports must be exists")
        }
    }

    fn add_top_level_var(&mut self, var_name: &str) -> bool {
        self.top_level_vars.insert(var_name.to_string())
    }

    fn setup_runtime_interops(&mut self, runtime_flags: RuntimeFlags) {
        for op in runtime_flags.iter() {
            let ident = self.request_safe_var_name(&op.op_ident());
            self.interop_module_items.push(op.dcl_with(&ident));
            self.interop_idents.insert(op, ident);
        }
    }
}
// why: it's a bug in swc before:
// https://github.com/swc-project/swc/blob/main/CHANGELOG.md#149---2024-03-26
fn collect_export_default_decl_ident(module: &Module) -> HashSet<Id> {
    let mut idents = HashSet::new();
    module.body.iter().for_each(|module_item| {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(ExportDefaultDecl {
            decl,
            ..
        })) = module_item
        {
            match decl {
                DefaultDecl::Class(ClassExpr {
                    ident: Some(ident), ..
                }) => {
                    idents.insert(ident.to_id());
                }
                DefaultDecl::Fn(FnExpr {
                    ident: Some(ident),
                    function: f,
                }) if f.body.is_some() => {
                    idents.insert(ident.to_id());
                }
                &_ => {}
            }
        }
    });
    idents
}

#[derive(Default)]
struct FnExprIdentCollector {
    idents: HashSet<Id>,
}

impl Visit for FnExprIdentCollector {
    fn visit_fn_expr(&mut self, fn_expr: &FnExpr) {
        if let Some(fn_ident) = fn_expr.ident.as_ref() {
            self.idents.insert(fn_ident.to_id());
        }

        fn_expr.visit_children_with(self);
    }
}

fn collect_named_fn_expr_ident(module: &Module) -> HashSet<Id> {
    let mut c = FnExprIdentCollector::default();
    module.visit_with(&mut c);

    c.idents
}

struct GlobalCollect {
    pub refed_globals: AHashSet<Id>,
    pub unresolved_ctxt: SyntaxContext,
}

impl GlobalCollect {
    pub fn new(unresolved_ctxt: SyntaxContext) -> Self {
        Self {
            unresolved_ctxt,
            refed_globals: AHashSet::default(),
        }
    }
}

impl Visit for GlobalCollect {
    fn visit_ident(&mut self, n: &Ident) {
        if n.span.ctxt == self.unresolved_ctxt {
            self.refed_globals.insert(n.to_id());
        }
    }
}

#[cfg(test)]
mod tests {

    use maplit::hashmap;
    use swc_core::common::GLOBALS;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_root_top_var_conflict_with_interop() {
        let mut context: ConcatenateContext = Default::default();
        context
            .top_level_vars
            .insert("_interop_require_default".to_string());

        context.setup_runtime_interops(RuntimeFlags::DefaultInterOp);

        assert_eq!(
            context
                .interop_idents
                .into_iter()
                .collect::<HashMap<RuntimeFlags, String>>(),
            hashmap! {
                RuntimeFlags::DefaultInterOp => "_interop_require_default_1".to_string()
            }
        )
    }

    #[test]
    fn test_export_default_class_expr_with_ident() {
        let tu = TestUtils::gen_js_ast("export default class C{};");
        let js = tu.ast.js();

        GLOBALS.set(&tu.context.meta.script.globals, || {
            assert!(ConcatenateContext::top_level_vars(&js.ast, js.top_level_mark).contains("C"));
        });
    }

    #[test]
    fn test_export_default_fn_expr_with_ident() {
        let tu = TestUtils::gen_js_ast("export default function fn(){};");
        let js = tu.ast.js();

        GLOBALS.set(&tu.context.meta.script.globals, || {
            assert!(ConcatenateContext::top_level_vars(&js.ast, js.top_level_mark).contains("fn"));
        });
    }

    #[test]
    fn test_export_default_anonymous_fn_expr_with_ident() {
        let tu = TestUtils::gen_js_ast("export default function (){};");
        let js = tu.ast.js();

        GLOBALS.set(&tu.context.meta.script.globals, || {
            assert!(ConcatenateContext::top_level_vars(&js.ast, js.top_level_mark).is_empty());
        });
    }

    #[test]
    fn test_export_default_anonymous_class_expr_with_ident() {
        let tu = TestUtils::gen_js_ast("export default class {};");
        let js = tu.ast.js();

        GLOBALS.set(&tu.context.meta.script.globals, || {
            assert!(ConcatenateContext::top_level_vars(&js.ast, js.top_level_mark).is_empty());
        });
    }
}
