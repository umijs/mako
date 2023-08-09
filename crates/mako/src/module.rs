use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use base64::engine::{general_purpose, Engine};
use pathdiff::diff_paths;

use crate::ast::Ast;
use crate::compiler::Context;
use crate::config::ModuleIdStrategy;

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
    pub raw_hash: u64,
    pub missing_deps: HashMap<String, ResolveType>,
}

impl ModuleInfo {}

fn md5_hash(source_str: &str, lens: usize) -> String {
    let digest = md5::compute(source_str);
    let hash = general_purpose::URL_SAFE.encode(digest.0);
    hash[..lens].to_string()
}

pub fn generate_module_id(origin_module_id: String, context: &Arc<Context>) -> String {
    match context.config.module_id_strategy {
        ModuleIdStrategy::Hashed => md5_hash(&origin_module_id, 4),
        ModuleIdStrategy::Named => {
            // readable ids for debugging usage
            // relative path to `&context.root`
            let absolute_path = PathBuf::from(origin_module_id);
            let relative_path = diff_paths(&absolute_path, &context.root).unwrap_or(absolute_path);
            // diff_paths result always starts with ".."/"." or not
            if relative_path.starts_with("..") || relative_path.starts_with(".") {
                relative_path.to_string_lossy().to_string()
            } else {
                PathBuf::from(".")
                    .join(relative_path)
                    .to_string_lossy()
                    .to_string()
            }
        }
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
    // we use absolute path as module id now
    pub fn new(id: String) -> Self {
        Self { id }
    }

    pub fn generate(&self, context: &Arc<Context>) -> String {
        // TODO: 如果是 Hashed 的话，stats 拿不到原始的 chunk_id
        generate_module_id(self.id.clone(), context)
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

impl From<&str> for ModuleId {
    fn from(id: &str) -> Self {
        Self { id: id.to_string() }
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
    Script(Ast),
    Css(swc_css_ast::Stylesheet),
    #[allow(dead_code)]
    None,
}

impl ModuleAst {
    pub fn as_script_mut(&mut self) -> &mut swc_ecma_ast::Module {
        if let Self::Script(script) = self {
            &mut script.ast
        } else {
            panic!("ModuleAst is not Script")
        }
    }
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
    pub is_missing: bool,
}
#[allow(dead_code)]

impl Module {
    pub fn new(id: ModuleId, is_entry: bool, info: Option<ModuleInfo>) -> Self {
        Self {
            id,
            is_entry,
            info,
            side_effects: false,
            is_missing: false,
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

    pub fn is_node_module(&self) -> bool {
        self.id.id.contains("node_modules")
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
