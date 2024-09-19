use std::collections::{HashMap, HashSet};

use swc_core::ecma::ast::{ComputedPropName, Id, Ident, Lit, MemberExpr, MemberProp};
use swc_core::ecma::visit::{Visit, VisitWith};

#[derive(Debug)]
pub struct IdExplicitPropAccessCollector {
    to_detected: HashSet<Id>,
    accessed_by_explicit_prop_count: HashMap<Id, usize>,
    ident_accessed_count: HashMap<Id, usize>,
    accessed_by: HashMap<Id, HashSet<String>>,
}

impl IdExplicitPropAccessCollector {
    pub(crate) fn new(ids: HashSet<Id>) -> Self {
        Self {
            to_detected: ids,
            accessed_by_explicit_prop_count: Default::default(),
            ident_accessed_count: Default::default(),
            accessed_by: Default::default(),
        }
    }
    pub(crate) fn explicit_accessed_props(mut self) -> HashMap<String, Vec<String>> {
        self.to_detected
            .iter()
            .filter_map(|id| {
                let member_prop_accessed = self.accessed_by_explicit_prop_count.get(id);
                let ident_accessed = self.ident_accessed_count.get(id);

                match (member_prop_accessed, ident_accessed) {
                    // all ident are accessed explicitly, so there is member expr there is a name
                    // ident, and at last plus the extra ident in import decl, that's 1 comes from.
                    (Some(m), Some(i)) if (i - m) == 1 => {
                        let mut accessed_by = Vec::from_iter(self.accessed_by.remove(id).unwrap());
                        accessed_by.sort();

                        let str_key = format!("{}#{}", id.0, id.1.as_u32());

                        Some((str_key, accessed_by))
                    }
                    // Some un-explicitly access e.g: obj[foo]
                    _ => None,
                }
            })
            .collect()
    }

    fn increase_explicit_prop_accessed_count(&mut self, id: Id) {
        self.accessed_by_explicit_prop_count
            .entry(id.clone())
            .and_modify(|c| {
                *c += 1;
            })
            .or_insert(1);
    }

    fn insert_member_accessed_by(&mut self, id: Id, prop: &str) {
        self.increase_explicit_prop_accessed_count(id.clone());
        self.accessed_by
            .entry(id)
            .and_modify(|accessed| {
                accessed.insert(prop.to_string());
            })
            .or_insert(HashSet::from([prop.to_string()]));
    }
}

impl Visit for IdExplicitPropAccessCollector {
    fn visit_ident(&mut self, n: &Ident) {
        let id = n.to_id();

        if self.to_detected.contains(&id) {
            self.ident_accessed_count
                .entry(id)
                .and_modify(|c| {
                    *c += 1;
                })
                .or_insert(1);
        }
    }

    fn visit_member_expr(&mut self, n: &MemberExpr) {
        if let Some(obj_ident) = n.obj.as_ident() {
            let id = obj_ident.to_id();

            if self.to_detected.contains(&id) {
                match &n.prop {
                    MemberProp::Ident(prop_ident) => {
                        self.insert_member_accessed_by(id, prop_ident.sym.as_ref());
                    }
                    MemberProp::PrivateName(_) => {}
                    MemberProp::Computed(ComputedPropName { expr, .. }) => {
                        if let Some(lit) = expr.as_lit()
                            && let Lit::Str(str) = lit
                        {
                            let visited_by = str.value.to_string();
                            self.insert_member_accessed_by(id, &visited_by)
                        }
                    }
                }
            }
        }

        n.visit_children_with(self);
    }
}

#[cfg(test)]
mod tests {
    use maplit::hashset;

    use super::*;
    use crate::ast::tests::TestUtils;

    #[test]
    fn test_no_prop() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        console.log(foo)
        "#,
        );

        assert_eq!(fields, None);
    }
    #[test]
    fn test_no_access() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        "#,
        );

        assert_eq!(fields, None);
    }

    #[test]
    fn test_computed_prop() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        foo['f' + 'o' + 'o']
        "#,
        );

        assert_eq!(fields, None);
    }

    #[test]
    fn test_simple_explicit_prop() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        foo.x;
        foo.y;
        "#,
        );

        assert_eq!(fields.unwrap(), vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn test_nest_prop_explicit_prop() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        foo.x.z[foo.y]
        "#,
        );

        assert_eq!(fields.unwrap(), vec!["x".to_string(), "y".to_string()]);
    }

    #[test]
    fn test_string_literal_prop_explicit() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        foo['x']
        "#,
        );

        assert_eq!(fields.unwrap(), vec!["x".to_string()]);
    }

    #[test]
    fn test_num_literal_prop_not_explicit() {
        let fields = extract_explicit_fields(
            r#"
        import * as foo from "./foo.js";
        foo[1]
        "#,
        );

        assert_eq!(fields, None);
    }

    fn extract_explicit_fields(code: &str) -> Option<Vec<String>> {
        let tu = TestUtils::gen_js_ast(code);

        let id = namespace_id(&tu);
        let str = format!("{}#{}", id.0, id.1.as_u32());

        let mut v = IdExplicitPropAccessCollector::new(hashset! { id });
        tu.ast.js().ast.visit_with(&mut v);

        v.explicit_accessed_props().remove(&str)
    }

    fn namespace_id(tu: &TestUtils) -> Id {
        tu.ast.js().ast.body[0]
            .as_module_decl()
            .unwrap()
            .as_import()
            .unwrap()
            .specifiers[0]
            .as_namespace()
            .unwrap()
            .local
            .to_id()
    }
}
