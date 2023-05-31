use crate::build::Task;
use crate::compiler::Compiler;
use crate::module::{Dependency, ModuleId};

use crate::resolve::get_resolver;

use nodejs_resolver::Resolver;
use rayon::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::fmt::Error;
use std::path::PathBuf;
use std::sync::Arc;

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
    pub modified: HashSet<ModuleId>,
}

impl Compiler {
    pub fn update(&self, paths: Vec<(PathBuf, UpdateType)>) -> Result<UpdateResult, Error> {
        let mut update_result = UpdateResult {
            ..Default::default()
        };
        let resolver = Arc::new(get_resolver(Some(
            self.context.config.resolve.alias.clone(),
        )));

        // 先分组
        let mut modified = vec![];
        let mut removed = vec![];
        let mut added = vec![];
        for (path, update_type) in paths {
            match update_type {
                UpdateType::Add => {
                    added.push(path);
                }
                UpdateType::Remove => {
                    removed.push(path);
                }
                UpdateType::Modify => {
                    modified.push(path);
                }
            }
        }

        // 先做删除
        let removed_module_ids = self.build_by_remove(removed);
        update_result.removed.extend(removed_module_ids);

        // 分析修改的模块，结果中会包含新增的模块
        let (modified_module_ids, add_paths) = self.build_by_modify(modified, resolver.clone());
        added.extend(add_paths);
        update_result.modified.extend(modified_module_ids);

        // 最后做添加
        let added_module_ids = self.build_by_add(added, resolver);
        update_result.added.extend(added_module_ids);

        Result::Ok(update_result)
    }

    fn build_by_modify(
        &self,
        modified: Vec<PathBuf>,
        resolver: Arc<Resolver>,
    ) -> (HashSet<ModuleId>, Vec<PathBuf>) {
        let result = modified
            .par_iter()
            .map(|entry| {
                // first build
                let (module, dependencies, _) = Compiler::build_module(
                    self.context.clone(),
                    Task {
                        path: entry.to_string_lossy().to_string(),
                        is_entry: false,
                    },
                    resolver.clone(),
                );

                // diff
                let module_graph = self.context.module_graph.read().unwrap();
                let current_dependencies: Vec<(ModuleId, Dependency)> = module_graph
                    .get_dependencies(&module.id)
                    .into_iter()
                    .map(|(module_id, dep)| (module_id.clone(), dep.clone()))
                    .collect();
                drop(module_graph);
                let target_dependencies: Vec<(ModuleId, Dependency)> = dependencies
                    .into_iter()
                    .map(|(path, _, dep)| (ModuleId::new(path), dep))
                    .collect();
                let (add, remove) = diff(current_dependencies, target_dependencies);
                (module, add, remove)
            })
            .collect::<Vec<_>>();

        // remove bind dependency
        for (module, _, remove) in &result {
            let mut module_graph = self.context.module_graph.write().unwrap();
            for (remove_module_id, _) in remove {
                module_graph.remove_dependency(remove_module_id, &module.id)
            }
        }

        // 把二维的结构拍平，如果有更好的写法可替换
        let mut added = vec![];
        let mut modified_module_ids = HashSet::new();
        for (module, add, _) in &result {
            // FIXME: 这里暂时直接通过 module_id 转换为 path，后续如果改了逻辑要记得改
            added.extend(add.iter().map(|f| PathBuf::from(f.0.id.clone())));
            modified_module_ids.insert(module.id.clone());
        }
        (modified_module_ids, added)
    }

    fn build_by_add(&self, added: Vec<PathBuf>, resolver: Arc<Resolver>) -> HashSet<ModuleId> {
        let mut add_queue: VecDeque<Task> = VecDeque::new();
        for path in added {
            add_queue.push_back(Task {
                path: path.to_string_lossy().to_string(),
                is_entry: true,
            })
        }

        self.build_module_graph_by_task_queue(&mut add_queue, resolver)
    }

    fn build_by_remove(&self, removed: Vec<PathBuf>) -> HashSet<ModuleId> {
        let mut removed_module_ids = HashSet::new();
        for path in removed {
            let from_module_id = ModuleId::from_path(path);
            let mut deps_module_ids = vec![];
            let mut module_graph = self.context.module_graph.write().unwrap();
            module_graph
                .get_dependencies(&from_module_id)
                .into_iter()
                .for_each(|(module_id, _)| {
                    deps_module_ids.push(module_id.clone());
                });
            for to_module_id in deps_module_ids {
                module_graph.remove_dependency(&from_module_id, &to_module_id);
            }
            module_graph.remove_module(&from_module_id);
            removed_module_ids.insert(from_module_id);
        }
        removed_module_ids
    }
}

// 对比两颗 Dependency 的差异
fn diff(
    right: Vec<(ModuleId, Dependency)>,
    left: Vec<(ModuleId, Dependency)>,
) -> (
    HashSet<(ModuleId, Dependency)>,
    HashSet<(ModuleId, Dependency)>,
) {
    let right: HashSet<(ModuleId, Dependency)> = right.into_iter().collect();
    let left: HashSet<(ModuleId, Dependency)> = left.into_iter().collect();
    let added = right
        .difference(&left)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|dep| (*dep).clone())
        .collect();
    let removed = left
        .difference(&right)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|dep| (*dep).clone())
        .collect();
    (added, removed)
}
