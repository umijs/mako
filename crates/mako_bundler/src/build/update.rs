use crate::build::build::Task;
use crate::compiler::Compiler;
use crate::module::ModuleId;

use std::collections::HashSet;
use std::fmt::Error;

pub enum UpdateType {
    Add,
    Remove,
    Modify,
}
#[derive(Default, Debug)]
pub struct UpdateResult {
    // 新增的模块Id
    pub added: HashSet<ModuleId>,
    // 删除的模块Id
    pub removed: HashSet<ModuleId>,
    // 修改的模块Id
    pub changed: HashSet<ModuleId>,
}

impl Compiler {
    pub fn update(&self, paths: Vec<(String, UpdateType)>) -> Result<UpdateResult, Error> {
        let mut update_result = UpdateResult {
            ..Default::default()
        };
        for (path, update_type) in paths {
            match update_type {
                UpdateType::Add => {
                    todo!()
                }
                UpdateType::Remove => {
                    todo!()
                }
                UpdateType::Modify => {
                    let path = self
                        .context
                        .config
                        .root
                        .join(path)
                        .to_string_lossy()
                        .to_string();
                    let walk_result = self.walk(Task {
                        parent_dependency: None,
                        parent_module_id: None,
                        path: path.clone(),
                    })?;
                    let path_str = path.as_str();
                    update_result.changed.insert(ModuleId::new(path_str));
                    update_result.added.extend(walk_result.added);
                    update_result.removed.extend(walk_result.removed);
                }
            }
        }
        Result::Ok(update_result)
    }
}
