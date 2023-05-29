use std::fmt::{Debug, Formatter};
use swc_common::{sync::Lrc, SourceMap};

#[derive(Debug, Clone)]
pub struct Dependency {
    pub source: String,
    pub resolve_type: ResolveType,
    pub order: usize,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ResolveType {
    Import,
    ExportNamed,
    ExportAll,
    Require,
    DynamicImport,
    Css,
}

pub struct ModuleInfo {
    pub ast: ModuleAst,
    pub path: String,
    pub external: Option<String>,
}

impl ModuleInfo {
    pub fn set_ast(&mut self, ast: ModuleAst) {
        self.ast = ast;
    }
}

// TODO:
// - id 不包含当前路径
// - 支持 hash id
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ModuleId {
    pub id: String,
}
impl ModuleId {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

pub enum ModuleAst {
    Script(swc_ecma_ast::Module),
    Css(swc_css_ast::Stylesheet),
    #[allow(dead_code)]
    None,
}

pub struct Module {
    pub id: ModuleId,
    pub is_entry: bool,
    pub info: Option<ModuleInfo>,
}

impl Module {
    pub fn new(id: ModuleId, is_entry: bool, info: Option<ModuleInfo>) -> Self {
        Self { id, is_entry, info }
    }

    #[allow(dead_code)]
    pub fn add_info(&mut self, info: Option<ModuleInfo>) {
        self.info = info;
    }
}

impl Debug for Module {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Module id={}", self.id.id)
    }
}
