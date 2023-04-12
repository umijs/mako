use std::collections::HashMap;

use crate::module::{Module, ModuleId};

pub struct Graph {}

pub struct ModuleGraph {
    pub id_module_map: HashMap<ModuleId, Module>,
    pub graph: Option<Graph>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self {
            id_module_map: HashMap::new(),
            graph: None,
        }
    }
}
