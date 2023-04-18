use lazy_static::lazy_static;
use std::sync::RwLock;
use swc_common::{sync::Lrc, SourceMap};

lazy_static! {
    static ref GLOBAL_ID: RwLock<usize> = RwLock::new(0);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub is_entry: bool,
}

pub struct ModuleTransformInfo {
    pub ast: ModuleAst,
    pub code: String,
}

pub struct Module {
    pub id: ModuleId,
    /**
     * 模块元信息
     */
    pub info: ModuleInfo,
    /**
     * 转换结果代码
     */
    pub transform_info: Option<ModuleTransformInfo>,
}

impl Module {
    pub fn new(id: ModuleId, info: ModuleInfo) -> Self {
        Self {
            id,
            info,
            transform_info: None,
        }
    }

    pub fn add_transform_info(&mut self, info: ModuleTransformInfo) {
        self.transform_info = Some(info);
    }
}
