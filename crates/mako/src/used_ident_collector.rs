use std::collections::HashSet;

use swc_ecma_ast::Ident;
use swc_ecma_visit::Visit;

/**
 * 收集所有使用到的标识符，由此可以分析出声明语句的依赖关系
 */
impl UsedIdentCollector {
    pub fn new() -> Self {
        Self {
            used_ident: HashSet::new(),
        }
    }
}

impl Visit for UsedIdentCollector {
    fn visit_ident(&mut self, ident: &Ident) {
        self.used_ident.insert(ident.to_string());
    }
}

pub struct UsedIdentCollector {
    pub used_ident: HashSet<String>,
}
