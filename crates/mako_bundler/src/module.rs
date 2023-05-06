use lazy_static::lazy_static;
use std::{collections::HashSet, sync::RwLock};
use swc_common::{sync::Lrc, SourceMap};

use crate::chunk::ChunkId;

lazy_static! {
    static ref GLOBAL_ID: RwLock<usize> = RwLock::new(0);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ModuleId {
    pub id: String,
}

impl ModuleId {
    pub fn new(path: &str) -> Self {
        let mut counter = GLOBAL_ID.write().unwrap();
        *counter += 1;
        Self {
            id: path.to_string(),
        }
        // Self {
        //     id: String::from_str(&(*counter.to_string())).unwrap(),
        // }
    }
}
#[derive(Clone)]
pub enum ModuleAst {
    Script(swc_ecma_ast::Module),
    Css(swc_css_ast::Stylesheet),
    None,
}

/**
 * 模块元信息
 */
pub struct ModuleInfo {
    pub original_ast: ModuleAst,
    pub original_cm: Option<Lrc<SourceMap>>,
    pub path: String,
    pub is_external: bool,
    pub external_name: Option<String>,
    pub is_entry: bool,
}

pub struct ModuleTransformInfo {
    pub ast: ModuleAst,
    pub code: Option<String>,
}

pub struct Module {
    pub id: ModuleId,
    /**
     * 模块元信息
     */
    pub info: Option<ModuleInfo>,

    /**
     * 当前模块归属的 chunk
     */
    pub chunks: HashSet<ChunkId>,
}

impl Module {
    pub fn new(id: ModuleId) -> Self {
        Self {
            id,
            info: None,
            chunks: HashSet::new(),
        }
    }

    pub fn add_info(&mut self, info: ModuleInfo) {
        self.info = Some(info);
    }
}
