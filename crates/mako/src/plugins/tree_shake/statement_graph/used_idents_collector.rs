use std::collections::HashSet;

use swc_core::ecma::ast::Ident;
use swc_core::ecma::visit::Visit;

pub struct UsedIdentsCollector {
    pub used_idents: HashSet<String>,
}

impl UsedIdentsCollector {
    pub fn new() -> Self {
        Self {
            used_idents: HashSet::new(),
        }
    }
}

impl Visit for UsedIdentsCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        self.used_idents.insert(ident.to_string());
    }
}
