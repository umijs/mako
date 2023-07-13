use std::collections::HashSet;

use swc_ecma_ast::{ObjectPatProp, Pat};
use swc_ecma_visit::{Visit, VisitWith};

use crate::used_ident_collector::UsedIdentCollector;

#[derive(Debug)]
pub struct DefinedIdentCollector {
    pub defined_ident: HashSet<String>,
    pub used_ident: HashSet<String>,
}

impl DefinedIdentCollector {
    pub fn new() -> Self {
        Self {
            defined_ident: HashSet::new(),
            used_ident: HashSet::new(),
        }
    }
}

impl Visit for DefinedIdentCollector {
    fn visit_pat(&mut self, pat: &Pat) {
        match pat {
            //
            Pat::Ident(bi) => {
                self.defined_ident.insert(bi.id.to_string());
            }
            // const [x, y] = [1, 2];
            Pat::Array(array_pat) => {
                for elem in array_pat.elems.iter().flatten() {
                    self.visit_pat(elem);
                }
            }
            // const [x, ...rest] = [1, 2, 3, 4];
            Pat::Rest(rest_pat) => {
                self.visit_pat(&rest_pat.arg);
            }
            // const { x, y } = { x: 1, y: 2 };
            Pat::Object(obj_pat) => {
                for prop in &obj_pat.props {
                    match prop {
                        ObjectPatProp::KeyValue(kv_prop) => {
                            self.visit_pat(&kv_prop.value);
                        }
                        ObjectPatProp::Assign(assign_prop) => {
                            self.defined_ident.insert(assign_prop.key.to_string());

                            let mut used_ident_collector = UsedIdentCollector::new();
                            assign_prop.value.visit_with(&mut used_ident_collector);

                            self.used_ident.extend(used_ident_collector.used_ident);
                        }
                        ObjectPatProp::Rest(rest_prop) => {
                            self.visit_pat(&rest_prop.arg);
                        }
                    }
                }
            }
            Pat::Assign(assign_pat) => {
                self.visit_pat(&assign_pat.left);
            }
            Pat::Invalid(_) => todo!(),
            Pat::Expr(_) => todo!(),
        }
    }
}
