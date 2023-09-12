use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Ok, Result};
use rayon::prelude::*;
use tracing::debug;

use crate::build::{get_entries, Task};
use crate::compiler::Compiler;
use crate::module::{Dependency, Module, ModuleId};
use crate::resolve::{self, get_resolvers, Resolvers};
use crate::transform_in_generate::transform_modules;

#[allow(dead_code)]
#[derive(Debug)]
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

impl UpdateResult {
    pub fn is_updated(&self) -> bool {
        !self.modified.is_empty() || !self.added.is_empty() || !self.removed.is_empty()
    }
}

impl fmt::Display for UpdateResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut added = self.added.iter().map(|f| f.id.clone()).collect::<Vec<_>>();
        added.sort_by_key(|id| id.to_string());
        let mut modified = self
            .modified
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        modified.sort_by_key(|id| id.to_string());
        let mut removed = self
            .removed
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        removed.sort_by_key(|id| id.to_string());
        write!(
            f,
            r#"
added:{:?}
modified:{:?}
removed:{:?}
"#,
            &added, &modified, &removed
        )
    }
}

impl Compiler {
    pub fn update(&self, paths: Vec<(PathBuf, UpdateType)>) -> Result<UpdateResult> {
        let mut update_result: UpdateResult = Default::default();
        let resolvers = Arc::new(get_resolvers(&self.context.config));

        let mut modified = vec![];
        let mut removed = vec![];
        let mut added = vec![];

        let mut has_added = false;
        for (path, update_type) in &paths {
            if matches!(update_type, UpdateType::Add) {
                debug!("has added {}", path.to_string_lossy());
                has_added = true;
                break;
            }
        }

        // try to resolve modules with missing deps
        // if found, add to modified queue
        if has_added {
            let mut modules_with_missing_deps =
                self.context.modules_with_missing_deps.write().unwrap();
            let mut module_graph = self.context.module_graph.write().unwrap();
            for module_id in modules_with_missing_deps.clone().iter() {
                let id = ModuleId::new(module_id.clone());
                let module = module_graph.get_module_mut(&id).unwrap();
                let missing_deps = module.info.clone().unwrap().missing_deps;
                for (_source, dep) in missing_deps {
                    let resolved =
                        resolve::resolve(module_id, &dep, &self.context.resolvers, &self.context);
                    if resolved.is_ok() {
                        debug!(
                            "missing deps resolved {:?} from {:?}",
                            dep.source, module_id
                        );
                        modified.push(PathBuf::from(module_id.clone()));
                        let info = module.info.as_mut().unwrap();
                        info.missing_deps.remove(&dep.source);
                        if info.missing_deps.is_empty() {
                            debug!("remove {} from modules_with_missing_deps", module_id);
                            modules_with_missing_deps.retain(|x| x == module_id);
                        }
                    }
                }
            }
        }

        // watch 到变化的文件，如果不在之前的 module graph 中，需过滤掉
        let paths: Vec<(PathBuf, UpdateType)> = {
            let module_graph = self.context.module_graph.read().unwrap();
            paths
                .into_iter()
                .filter(|(p, _)| module_graph.has_module(&p.clone().into()))
                .collect()
        };

        // 先分组
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
        let (removed_module_ids, affected_module_ids) = self.build_by_remove(removed);
        update_result.removed.extend(removed_module_ids);

        modified.extend(affected_module_ids.into_iter().map(|i| i.to_path()));

        // 分析修改的模块，结果中会包含新增的模块
        let (modified_module_ids, add_paths) = self
            .build_by_modify(modified, resolvers.clone())
            .map_err(|err| anyhow!(err))?;

        added.extend(add_paths);
        debug!("added:{:?}", &added);
        update_result.modified.extend(modified_module_ids);

        // 最后做添加
        let added_module_ids = self.build_by_add(&added, resolvers);
        update_result.added.extend(
            added
                .into_iter()
                .map(ModuleId::from_path)
                .collect::<HashSet<_>>(),
        );
        update_result.added.extend(added_module_ids);
        debug!("update_result:{:?}", &update_result);

        Result::Ok(update_result)
    }

    pub fn transform_for_change(&self, update_result: &UpdateResult) -> Result<()> {
        let mut changes: Vec<ModuleId> = vec![];
        for module_id in &update_result.added {
            changes.push(module_id.clone());
        }
        for module_id in &update_result.modified {
            changes.push(module_id.clone());
        }
        transform_modules(changes, &self.context)?;
        Ok(())
    }

    fn build_by_modify(
        &self,
        mut modified: Vec<PathBuf>,
        resolvers: Arc<Resolvers>,
    ) -> Result<(HashSet<ModuleId>, Vec<PathBuf>)> {
        let module_graph = self.context.module_graph.read().unwrap();
        let modules = module_graph.modules();

        // concat related query modules for modified paths
        // for example: concat a.module.css?modules for a.module.css
        for module in modules
            .iter()
            .filter(|module| module.id.id.contains("?modules"))
        {
            let origin_id = module.id.id.split('?').next().unwrap();

            if modified.contains(&PathBuf::from(origin_id)) {
                modified.push(PathBuf::from(module.id.id.clone()));
            }
        }
        drop(module_graph);

        let result = modified
            .par_iter()
            .map(|entry| {
                // first build

                let is_entry = {
                    // there must be a entry, so unwrap is safe
                    let entries = get_entries(&self.context.root, &self.context.config).unwrap();
                    entries.contains(entry)
                };

                let (module, dependencies, _) = Compiler::build_module(
                    self.context.clone(),
                    Task {
                        path: entry.to_string_lossy().to_string(),
                        is_entry,
                        parent_resource: None,
                    },
                    resolvers.clone(),
                )?;

                // update modules_with_missing_deps
                if module.info.clone().unwrap().missing_deps.is_empty() {
                    self.context
                        .modules_with_missing_deps
                        .write()
                        .unwrap()
                        .retain(|id| id != &module.id.id);
                } else {
                    self.context
                        .modules_with_missing_deps
                        .write()
                        .unwrap()
                        .push(module.id.id.clone());
                }

                // diff
                let module_graph = self.context.module_graph.read().unwrap();
                let current_dependencies: Vec<(ModuleId, Dependency)> = module_graph
                    .get_dependencies(&module.id)
                    .into_iter()
                    .map(|(module_id, dep)| (module_id.clone(), dep.clone()))
                    .collect();
                drop(module_graph);

                let mut add_modules: HashMap<ModuleId, Module> = HashMap::new();
                let mut target_dependencies: Vec<(ModuleId, Dependency)> = vec![];
                dependencies.into_iter().for_each(|(resource, dep)| {
                    let resolved_path = resource.get_resolved_path();
                    let module_id = ModuleId::new(resolved_path);
                    // TODO: handle error
                    let module = self.create_module(&resource, &module_id).unwrap();
                    target_dependencies.push((module_id.clone(), dep));
                    add_modules.insert(module_id, module);
                });

                let d = diff(current_dependencies, target_dependencies);
                Result::Ok((module, d.added, d.removed, add_modules))
            })
            .collect::<Result<Vec<_>>>();
        let result = result?;

        let mut added = vec![];
        let mut modified_module_ids = HashSet::new();

        let mut module_graph = self.context.module_graph.write().unwrap();
        for (module, add, remove, mut add_modules) in result {
            // remove bind dependency
            for (remove_module_id, _) in remove {
                module_graph.remove_dependency(&module.id, &remove_module_id)
            }

            // add bind dependency
            for (add_module_id, dep) in &add {
                let add_module = add_modules.remove(add_module_id).unwrap();

                // 只针对非 external 的模块设置 add Task
                if add_module.info.is_none() {
                    added.push(add_module_id.to_path());
                }

                module_graph.add_module(add_module);
                module_graph.add_dependency(&module.id, add_module_id, dep.clone());
            }

            modified_module_ids.insert(module.id.clone());

            // replace module
            module_graph.replace_module(module);
        }

        Result::Ok((modified_module_ids, added))
    }

    fn build_by_add(&self, added: &Vec<PathBuf>, resolvers: Arc<Resolvers>) -> HashSet<ModuleId> {
        let mut add_queue: VecDeque<Task> = VecDeque::new();
        for path in added {
            add_queue.push_back(Task {
                path: path.to_string_lossy().to_string(),
                is_entry: false,
                parent_resource: None,
            })
        }

        self.build_module_graph_by_task_queue(&mut add_queue, resolvers)
    }

    fn build_by_remove(&self, removed: Vec<PathBuf>) -> (HashSet<ModuleId>, HashSet<ModuleId>) {
        let mut removed_module_ids = HashSet::new();
        let mut affected_module_ids = HashSet::new();
        for path in removed {
            let from_module_id = ModuleId::from_path(path);

            let module_graph = self.context.module_graph.write().unwrap();
            let dependants = module_graph.dependant_module_ids(&from_module_id);
            affected_module_ids.extend(dependants);
            removed_module_ids.insert(from_module_id);
        }
        (removed_module_ids, affected_module_ids)
    }
}

pub struct Diff {
    added: HashSet<(ModuleId, Dependency)>,
    removed: HashSet<(ModuleId, Dependency)>,
}

// 对比两颗 Dependency 的差异
fn diff(origin: Vec<(ModuleId, Dependency)>, target: Vec<(ModuleId, Dependency)>) -> Diff {
    let origin_module_ids = origin
        .iter()
        .map(|(module_id, _dep)| module_id)
        .collect::<HashSet<_>>();
    let target_module_ids = target
        .iter()
        .map(|(module_id, _dep)| module_id)
        .collect::<HashSet<_>>();
    let mut added: HashSet<(ModuleId, Dependency)> = HashSet::new();
    let mut removed: HashSet<(ModuleId, Dependency)> = HashSet::new();
    target
        .iter()
        .filter(|(module_id, _dep)| !origin_module_ids.contains(module_id))
        .for_each(|(module_id, dep)| {
            added.insert((module_id.clone(), dep.clone()));
        });
    origin
        .iter()
        .filter(|(module_id, _dep)| !target_module_ids.contains(module_id))
        .for_each(|(module_id, dep)| {
            removed.insert((module_id.clone(), dep.clone()));
        });
    Diff { added, removed }
}

#[cfg(test)]
mod tests {

    use crate::module::ModuleId;
    use crate::test_helper::{module_to_jscode, setup_compiler, setup_files};
    use crate::update::UpdateType;
    use crate::{assert_debug_snapshot, assert_display_snapshot};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_build() {
        let compiler = setup_compiler("test/build/tmp/single", true);
        setup_files(
            &compiler,
            vec![
                (
                    "mako.config.json".into(),
                    r#"{"mode": "production"}"#.into(),
                ),
                (
                    "index.ts".into(),
                    r#"
(async () => {
    await import('./chunk-1.ts');
})();
    "#
                    .into(),
                ),
                (
                    "chunk-1.ts".into(),
                    r#"
export default async function () {
    console.log(123);
}
    "#
                    .into(),
                ),
            ],
        );
        compiler.compile();
        {
            let module_graph = compiler.context.module_graph.read().unwrap();
            assert_display_snapshot!(&module_graph);
        }
        setup_files(
            &compiler,
            vec![
                (
                    "index.ts".into(),
                    r#"
(async () => {
    await import('./chunk-2.ts');
})();
"#
                    .into(),
                ),
                (
                    "chunk-2.ts".into(),
                    r#"
export const foo = 1;
"#
                    .into(),
                ),
            ],
        );
        let result = compiler
            .update(vec![(
                compiler.context.root.join("index.ts"),
                UpdateType::Modify,
            )])
            .unwrap();

        assert_display_snapshot!(&result);

        {
            let module_graph = compiler.context.module_graph.read().unwrap();
            assert_display_snapshot!(&module_graph);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_update_multi() {
        let compiler = setup_compiler("test/build/tmp/multi", true);
        let target_path = compiler.context.root.join("index.ts");
        setup_files(
            &compiler,
            vec![
                (
                    "mako.config.json".into(),
                    r#"{"mode": "production"}"#.into(),
                ),
                (
                    "index.ts".into(),
                    r#"
(async () => {
    await import('./chunk-1.ts');
})();
    "#
                    .into(),
                ),
                (
                    "chunk-1.ts".into(),
                    r#"
export default async function () {
    console.log(123);
}
    "#
                    .into(),
                ),
            ],
        );
        compiler.compile();
        {
            let module_graph = compiler.context.module_graph.read().unwrap();
            let code = module_to_jscode(&compiler, &ModuleId::from_path(target_path.clone()));
            assert_display_snapshot!(&module_graph);
            assert_debug_snapshot!(&code);
        }
        setup_files(
            &compiler,
            vec![
                (
                    "index.ts".into(),
                    r#"
(async () => {
    await import('./chunk-2.ts');
})();
"#
                    .into(),
                ),
                (
                    "chunk-2.ts".into(),
                    r#"
export * from './chunk-3.ts';
"#
                    .into(),
                ),
                (
                    "chunk-3.ts".into(),
                    r#"
export const foo = 1;
"#
                    .into(),
                ),
            ],
        );
        let result = compiler
            .update(vec![(target_path.clone(), UpdateType::Modify)])
            .unwrap();

        assert_display_snapshot!(&result);
        {
            compiler.generate_hot_update_chunks(result, 0).unwrap();

            let module_graph = compiler.context.module_graph.read().unwrap();
            let code = module_to_jscode(&compiler, &ModuleId::from_path(target_path));
            assert_display_snapshot!(&module_graph);
            assert_debug_snapshot!(&code);
        }
        {
            let module_graph = compiler.context.module_graph.read().unwrap();
            assert_display_snapshot!(&module_graph);
        }
    }
}
