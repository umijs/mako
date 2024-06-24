use std::fmt::{Display, Formatter};

use swc_core::ecma::visit::VisitWith;

use super::*;
use crate::ast::tests::TestUtils;

#[test]
fn import_default() {
    assert_eq!(
        extract_import_map("import a from 'src'"),
        "a => Default from src"
    )
}

#[test]
fn import_named() {
    assert_eq!(
        extract_import_map("import {a,b} from 'src'"),
        r#"a => a from src
b => b from src"#
    )
}

#[test]
fn import_named_as() {
    assert_eq!(
        extract_import_map(
            r#"import {a as x} from 'foo';
        import {b as y} from './src'"#
        ),
        r#"x => a from foo
y => b from ./src"#
    )
}

#[test]
fn export_default_expr() {
    assert_eq!(
        extract_export_map("export default 1"),
        "default => _$m_default_name_binding"
    );
}

#[test]
fn export_named_fn() {
    assert_eq!(extract_export_map("export function fn(){}"), "fn => fn");
}

#[test]
fn export_named_class() {
    assert_eq!(extract_export_map("export class C{}"), "C => C");
}

#[test]
fn export_names() {
    assert_eq!(
        extract_export_map("let a=1,b=2; export {a,b}"),
        "a => a\nb => b"
    );
}

#[test]
fn export_names_as() {
    assert_eq!(extract_export_map("let a=1,b=2; export {a as x}"), "x => a");
}

#[test]
fn export_names_from_source() {
    assert_eq!(
        extract_export_map("export {a,b} from 'external'"),
        "a => a from external\nb => b from external"
    );
}

#[test]
fn export_default_from_source() {
    assert_eq!(
        extract_export_map("export { default } from 'external'"),
        "default => Default from external"
    );
}

#[test]
fn export_default_as_from_source() {
    assert_eq!(
        extract_export_map("export { default as foo } from 'external'"),
        "foo => Default from external"
    );
}

#[test]
fn export_namespace_as_from_source() {
    assert_eq!(
        extract_export_map("export * as foo from 'external'"),
        "foo => * from external"
    );
}

#[test]
fn export_name_as_from_source() {
    assert_eq!(
        extract_export_map("export {a as x} from 'external'"),
        "x => a from external"
    );
}

#[test]
fn export_all_from_source_at_first_stmt() {
    assert_eq!(
        extract_export_map("export * from 'source'"),
        "*:0 => * from source"
    );
}
#[test]

fn export_all_from_source_at_second_stmt() {
    assert_eq!(
        extract_export_map("import x from 'mod'; export * from 'source'"),
        "*:1 => * from source"
    );
}

#[test]
fn export_object_deconstruct() {
    assert_eq!(
        extract_export_map("let A= {a:1,b:2, c:3}; export const {a,b:x, ...z} = A"),
        r#"a => a
x => x
z => z"#
    );
}

#[test]
fn export_array_deconstruct() {
    assert_eq!(
        extract_export_map("let a= [1,2,3]; export const [x,y,...z] = a"),
        r#"x => x
y => y
z => z"#
    );
}

#[test]
fn export_var_decl_export() {
    assert_eq!(extract_export_map("export const a =1"), "a => a");
}

#[test]
fn simplify_exports_map() {
    assert_eq!(
        extract_export_map(r#"import x from "src"; export {x as foo}"#),
        "foo => Default from src"
    );
}

fn extract_export_map(code: &str) -> String {
    let mut ast = TestUtils::gen_js_ast(code);
    let mut c = ModuleDeclMapCollector::new("_$m_default_name_binding".to_string());

    ast.ast.js_mut().ast.visit_with(&mut c);
    c.simplify_exports();
    map_to_string(&c.export_map)
}

fn extract_import_map(code: &str) -> String {
    let mut ast = TestUtils::gen_js_ast(code);
    let mut c = ModuleDeclMapCollector::default();

    ast.ast.js_mut().ast.visit_with(&mut c);
    map_to_string(&c.import_map)
}

fn map_to_string(import_map: &HashMap<Id, VarLink>) -> String {
    let mut result = String::new();

    let mut sorted_ids = import_map.keys().cloned().collect::<Vec<_>>();
    sorted_ids.sort_by_key(|ident| ident.0.to_string());

    for ident in sorted_ids {
        result.push_str(&format!("{} => {}\n", ident.0, import_map[&ident]));
    }
    result.trim().to_string()
}

impl Display for VarLink {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VarLink::Direct(id) => write!(f, "{}", id.0),
            VarLink::InDirect(symbol, source) => write!(f, "{} from {}", symbol, source),
            VarLink::All(source, _) => {
                write!(f, "* from {}", source)
            }
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Symbol::Default => f.write_str("Default"),
            Symbol::Namespace => f.write_str("*"),
            Symbol::Var(ident) => f.write_str(&ident.sym),
        }
    }
}
