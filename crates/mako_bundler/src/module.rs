use std::sync::RwLock;

use lazy_static::lazy_static;

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

pub struct ModuleInfo {
    pub code: String,
    pub ast: ModuleAst,
    pub path: String,
    pub is_external: bool,
    pub is_entry: bool,
}

pub struct Module {
    pub id: ModuleId,
    pub info: ModuleInfo,
}

impl Module {
    pub fn new(id: ModuleId, info: ModuleInfo) -> Self {
        Self { id, info }
    }
}
