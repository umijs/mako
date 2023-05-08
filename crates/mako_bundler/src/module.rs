use lazy_static::lazy_static;
use std::{collections::HashSet, sync::RwLock};
use swc_common::{sync::Lrc, SourceMap};

use crate::chunk::ChunkId;
use crate::module_graph::Dependency;

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

pub enum ModuleAst {
    Script(swc_ecma_ast::Module),
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

pub enum ModuleInfo2 {
    External {
        module_id: ModuleId,
        path: String,
        external_name: String,
        dep: Dependency,
    },
    Normal {
        module_id: ModuleId,
        path: String,
        original_ast: ModuleAst,
        original_cm: Lrc<SourceMap>,
        is_entry: bool,
    },
}

impl ModuleInfo2 {
    pub fn path(&self) -> String {
        match self {
            ModuleInfo2::External {
                module_id,
                path: _,
                external_name: _,
                dep: _dep,
            } => module_id.id.clone(),
            ModuleInfo2::Normal {
                module_id,
                path: _,
                original_ast: _,
                original_cm: _,
                is_entry: _is_entry,
            } => module_id.id.clone(),
        }
    }

    pub fn module_id(&self) -> &ModuleId {
        match self {
            ModuleInfo2::External {
                module_id,
                path: _,
                external_name: _,
                dep: _dep,
            } => module_id,
            ModuleInfo2::Normal {
                module_id,
                path: _,
                original_ast: _,
                original_cm: _,
                is_entry: _is_entry,
            } => module_id,
        }
    }
}

impl From<ModuleInfo2> for ModuleInfo {
    fn from(module_info: ModuleInfo2) -> ModuleInfo {
        match module_info {
            ModuleInfo2::Normal {
                module_id: _module_id,
                is_entry,
                path,
                original_ast,
                original_cm,
            } => ModuleInfo {
                is_entry,
                original_ast,
                external_name: None,
                path,
                original_cm: Some(original_cm),
                is_external: false,
            },

            ModuleInfo2::External {
                path,
                dep: _dep,
                module_id: _module_id,
                external_name,
            } => ModuleInfo {
                path,
                is_external: true,
                is_entry: false,
                original_ast: ModuleAst::None,
                original_cm: None,
                external_name: Some(external_name),
            },
        }
    }
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
