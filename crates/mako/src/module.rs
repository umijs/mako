use std::fmt::{Debug, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Dependency {
    pub source: String,
    pub resolve_type: ResolveType,
    pub order: usize,
}

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub enum ResolveType {
    Import,
    ExportNamed,
    ExportAll,
    Require,
    DynamicImport,
    Css,
}

#[derive(Debug)]
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

impl Ord for ModuleId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for ModuleId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl ModuleId {
    pub fn new(id: String) -> Self {
        Self { id }
    }

    pub fn from_path(path_buf: PathBuf) -> Self {
        Self {
            id: path_buf.to_string_lossy().to_string(),
        }
    }

    // FIXME: 这里暂时直接通过 module_id 转换为 path，后续如果改了逻辑要记得改
    pub fn to_path(&self) -> PathBuf {
        PathBuf::from(self.id.clone())
    }
}

impl From<String> for ModuleId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl From<PathBuf> for ModuleId {
    fn from(path: PathBuf) -> Self {
        Self {
            id: path.to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ModuleAst {
    Script(swc_ecma_ast::Module),
    Css(swc_css_ast::Stylesheet),
    #[allow(dead_code)]
    None,
}

#[allow(dead_code)]
pub enum ModuleType {
    Script,
    Css,
}

#[allow(dead_code)]
impl ModuleType {
    pub fn is_script(&self) -> bool {
        matches!(self, ModuleType::Script)
    }
}
#[allow(dead_code)]

pub struct Module {
    pub id: ModuleId,
    pub is_entry: bool,
    pub info: Option<ModuleInfo>,
    pub side_effects: bool,
}
#[allow(dead_code)]

impl Module {
    pub fn new(id: ModuleId, is_entry: bool, info: Option<ModuleInfo>) -> Self {
        Self {
            id,
            is_entry,
            info,
            side_effects: false,
        }
    }

    #[allow(dead_code)]
    pub fn add_info(&mut self, info: Option<ModuleInfo>) {
        self.info = info;
    }

    pub fn is_external(&self) -> bool {
        let info = self.info.as_ref().unwrap();
        info.external.is_some()
    }

    pub fn get_module_type(&self) -> ModuleType {
        let info = self.info.as_ref().unwrap();
        match info.ast {
            ModuleAst::Script(_) => ModuleType::Script,
            ModuleAst::Css(_) => ModuleType::Css,
            ModuleAst::None => todo!(),
        }
    }
}

impl Debug for Module {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Module id={}", self.id.id)
    }
}
