use mako_core::collections::HashSet;
use mako_core::swc_ecma_ast::Ident;
use mako_core::swc_ecma_visit::Visit;

pub struct UsedIdentsCollector {
    pub used_idents: HashSet<String>,
}

impl UsedIdentsCollector {
    pub fn new() -> Self {
        Self {
            used_idents: HashSet::default(),
        }
    }
}

impl Visit for UsedIdentsCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        self.used_idents.insert(ident.to_string());
    }
}
