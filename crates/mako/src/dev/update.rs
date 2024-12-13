use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Debug;
use std::path::PathBuf;

use anyhow::{anyhow, Ok, Result};
use rayon::prelude::*;
use tracing::debug;

use crate::ast::file::File;
use crate::build::BuildError;
use crate::compiler::Compiler;
use crate::generate::transform::transform_modules;
use crate::module::{Dependency, Module, ModuleId, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugin::NextBuildParam;
use crate::resolve::{self, clear_resolver_cache};

#[derive(Debug, Clone)]
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
    // 依赖变更，典型的如 async import 变成 import
    pub dep_changed: HashSet<ModuleId>,
}

impl UpdateResult {
    pub fn is_updated(&self) -> bool {
        !self.modified.is_empty()
            || !self.added.is_empty()
            || !self.removed.is_empty()
            || !self.dep_changed.is_empty()
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
        let mut dep_changed = self
            .dep_changed
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        dep_changed.sort_by_key(|id| id.to_string());
        write!(
            f,
            r#"
added:{:?}
modified:{:?}
removed:{:?}
dep_changed:{:?}
"#,
            &added, &modified, &removed, &dep_changed
        )
    }
}

impl Compiler {
    pub fn update(&self, paths: Vec<PathBuf>) -> Result<UpdateResult> {
        let module_graph = self.context.module_graph.read().unwrap();
        let paths = paths
            .into_iter()
            .map(|path| {
                let update_type = if path.exists() {
                    let path = path.to_string_lossy().to_string();
                    if module_graph.has_module(&path.clone().into())
                        || module_graph.has_module(&format!("{}?modules", path).into())
                        || module_graph.has_module(&format!("{}?watch=parent", path).into())
                    {
                        UpdateType::Modify
                    } else {
                        UpdateType::Add
                    }
                } else {
                    UpdateType::Remove
                };
                (path, update_type)
            })
            .collect::<Vec<_>>();
        drop(module_graph);
        debug!("update: {:?}", &paths);
        let mut update_result: UpdateResult = Default::default();

        let mut modified = vec![];
        let mut removed = vec![];
        let mut added = vec![];

        debug!("checking added...");
        let mut has_added = false;
        for (path, update_type) in &paths {
            if matches!(update_type, UpdateType::Add) {
                debug!("  > {} is added", path.to_string_lossy());
                has_added = true;
                break;
            }
        }
        debug!("checking added...done, has_added:{}", has_added);

        // try to resolve modules with missing deps
        // if found, add to modified queue
        if has_added {
            debug!("checking modules_with_missing_deps... since has added modules");
            // clear resolver cache before resolving to avoid wrong result, i.e. add missing dep after watch started
            clear_resolver_cache(&self.context.resolvers);
            let mut modules_with_missing_deps =
                self.context.modules_with_missing_deps.write().unwrap();
            let mut module_graph = self.context.module_graph.write().unwrap();
            for module_id in modules_with_missing_deps.clone().iter() {
                let id = ModuleId::new(module_id.clone());
                let module = module_graph.get_module_mut(&id).unwrap();
                let missing_deps = module.info.clone().unwrap().deps.missing_deps;
                for (_source, dep) in missing_deps {
                    let resolved =
                        resolve::resolve(module_id, &dep, &self.context.resolvers, &self.context);
                    if resolved.is_ok() {
                        debug!(
                            "  > missing deps resolved {:?} from {:?}",
                            dep.source, module_id
                        );
                        modified.push(PathBuf::from(module_id.clone()));
                        let info = module.info.as_mut().unwrap();
                        info.deps.missing_deps.remove(&dep.source);
                        if info.deps.missing_deps.is_empty() {
                            debug!("  > remove {} from modules_with_missing_deps", module_id);
                            modules_with_missing_deps.retain(|x| x == module_id);
                        }
                    }
                }
            }
            debug!("checking modules_with_missing_deps...done");
        }

        // watch 到变化的文件，如果不在之前的 module graph 中，需过滤掉
        debug!("filtering paths...");
        let paths: Vec<(PathBuf, UpdateType)> = {
            let module_graph = self.context.module_graph.read().unwrap();
            let mut new_paths = vec![];
            paths.into_iter().for_each(|(p, update_type)| {
                if module_graph.has_module(&p.clone().into()) {
                    debug!("  > {} is filtered", p.to_string_lossy());
                    new_paths.push((p.clone(), update_type.clone()));
                }
                let path = p.to_string_lossy().to_string();
                let watch_parent_searches = vec!["?modules", "?watch=parent"];
                for search in watch_parent_searches {
                    let id: ModuleId = format!("{}{}", path, search).into();
                    if module_graph.has_module(&id) {
                        debug!("  > {} is filtered", &id.id);
                        new_paths.push((PathBuf::from(&id.id), update_type.clone()));
                        let dependents = module_graph.get_dependents(&id);
                        for dependent in dependents {
                            debug!("  > {} is filtered", dependent.0.id);
                            new_paths
                                .push((PathBuf::from(dependent.0.id.clone()), update_type.clone()));
                        }
                    }
                }
            });
            new_paths
        };
        debug!("filtering paths...done");

        // 先分组
        for (path, update_type) in paths {
            match update_type {
                UpdateType::Add => {
                    self.context.plugin_driver.watch_changes(
                        &path.to_string_lossy(),
                        "create",
                        &self.context,
                    )?;
                    added.push(path);
                }
                UpdateType::Remove => {
                    self.context.plugin_driver.watch_changes(
                        &path.to_string_lossy(),
                        "delete",
                        &self.context,
                    )?;
                    removed.push(path);
                }
                UpdateType::Modify => {
                    self.context.plugin_driver.watch_changes(
                        &path.to_string_lossy(),
                        "update",
                        &self.context,
                    )?;
                    modified.push(path);
                }
            }
        }

        // 先做删除
        debug!("remove: {:?}", &removed);
        let (removed_module_ids, affected_module_ids) = self.build_by_remove(removed);
        debug!("after build_by_remove");
        debug!("  > removed_module_ids: {:?}", &removed_module_ids);
        debug!(
            "  > affected_module_ids: {:?} (these will be added to modified",
            &affected_module_ids
        );
        update_result.removed.extend(removed_module_ids);
        modified.extend(affected_module_ids.into_iter().map(|i| i.to_path()));

        // 分析修改的模块，结果中会包含新增的模块
        debug!("modify: {:?}", &modified);
        let (modified_module_ids, dep_changed_module_ids, add_paths) =
            self.build_by_modify(modified).map_err(|err| anyhow!(err))?;
        debug!("after build_by_modify");
        debug!("  > modified_module_ids: {:?}", &modified_module_ids);
        debug!(
            "  > add_paths: {:?} (these will be added to added)",
            &add_paths
        );

        added.extend(add_paths);

        update_result.modified.extend(modified_module_ids);

        update_result.dep_changed.extend(dep_changed_module_ids);

        // 最后做添加
        debug!("add: {:?}", &added);
        let added_module_ids = self.build_by_add(&added)?;
        update_result.added.extend(
            added
                .into_iter()
                .map(ModuleId::from_path)
                .collect::<HashSet<_>>(),
        );
        update_result.added.extend(added_module_ids);

        debug!("update_result: {:?}", &update_result);

        self.context.plugin_driver.after_update(self)?;

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
        modified: Vec<PathBuf>,
    ) -> Result<(HashSet<ModuleId>, HashSet<ModuleId>, Vec<PathBuf>)> {
        let result = modified
            .par_iter()
            .map(|entry| {
                debug!("build by modify: {:?} start", entry);
                // first build
                let is_entry = {
                    let mut entries = self.context.config.entry.values();
                    entries.any(|e| e.eq(entry))
                };

                let path = entry.to_string_lossy().to_string();
                let file = if is_entry {
                    crate::ast::file::File::new_entry(path, self.context.clone())
                } else {
                    crate::ast::file::File::new(path, self.context.clone())
                };
                let module = Self::build_module(&file, None, self.context.clone())
                    .map_err(|err| BuildError::BuildTasksError { errors: vec![err] })?;

                debug!(
                    "  > missing deps: {:?}",
                    module.info.as_ref().unwrap().deps.missing_deps
                );

                // update modules_with_missing_deps
                if module.info.as_ref().unwrap().deps.missing_deps.is_empty() {
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

                let mut dependence_modules: HashMap<ModuleId, Module> = HashMap::new();
                let mut target_dependencies: Vec<(ModuleId, Dependency)> = vec![];
                let resolved_deps = &module.info.as_ref().unwrap().deps.resolved_deps;
                resolved_deps.iter().for_each(|dep| {
                    let resolved_path = dep.resolver_resource.get_resolved_path();
                    let is_external = dep.resolver_resource.get_external().is_some();
                    let dep_module_id = ModuleId::new(resolved_path.clone());
                    let dep_module = if is_external {
                        Self::create_external_module(&dep.resolver_resource, self.context.clone())
                    } else {
                        Self::create_empty_module(&dep_module_id)
                    };
                    target_dependencies.push((dep_module_id.clone(), dep.dependency.clone()));
                    dependence_modules.insert(dep_module_id, dep_module);

                    self.context.plugin_driver.next_build(&NextBuildParam {
                        current_module: &module.id,
                        next_file: &File::new(resolved_path.clone(), self.context.clone()),
                        resource: &dep.resolver_resource,
                    });
                });

                let modules_diff = diff(&current_dependencies, &target_dependencies);

                debug!("build by modify: {:?} end", entry);
                Result::Ok((
                    module,
                    modules_diff,
                    dependence_modules,
                    target_dependencies,
                ))
            })
            .collect::<Result<Vec<_>>>();
        let modified_results = result?;

        let mut added = vec![];
        let mut modified_module_ids = HashSet::new();
        let mut dep_changed_module_ids = HashSet::new();

        let mut module_graph = self.context.module_graph.write().unwrap();
        for (modified_module, diff, mut dependence_modules, dependencies) in modified_results {
            if diff.dependence_changed(&modified_module.id, &module_graph, &dependencies) {
                dep_changed_module_ids.insert(modified_module.id.clone());
            }

            // remove bind dependency
            for remove_module_id in &diff.removed {
                module_graph.clear_dependency(&modified_module.id, remove_module_id);
            }

            // add bind dependency
            for add_module_id in &diff.added {
                let mut deps = dependencies
                    .iter()
                    .filter(|(module_id, _dep)| module_id == add_module_id)
                    .map(|(_, dep)| dep)
                    .collect::<Vec<_>>();
                deps.sort_by_key(|d| d.order);

                // english: In theory, the add_module_id that add_modules should exist in must exist, but in actual scenarios, an unwrap() error still occurs, so add a guard check here
                // TODO: Need to find the root cause
                let add_module = dependence_modules.remove(add_module_id);
                if add_module.is_none() {
                    continue;
                }
                let add_module = add_module.unwrap();

                // 只针对非 external 的模块设置 add Task
                if add_module.info.is_none() {
                    added.push(add_module_id.to_path());
                }

                module_graph.add_module(add_module);

                deps.iter().for_each(|&dep| {
                    module_graph.add_dependency(&modified_module.id, add_module_id, dep.clone());
                });
            }

            diff.modified.iter().for_each(|to_module_id| {
                let deps = dependencies
                    .iter()
                    .filter(|(module_id, _dep)| module_id == to_module_id)
                    .map(|(_, dep)| dep)
                    .collect::<Vec<_>>();

                module_graph.clear_dependency(&modified_module.id, to_module_id);

                deps.iter().for_each(|&dep| {
                    module_graph.add_dependency(&modified_module.id, to_module_id, dep.clone());
                });
            });

            modified_module_ids.insert(modified_module.id.clone());

            // replace module
            module_graph.replace_module(modified_module);
        }

        Result::Ok((modified_module_ids, dep_changed_module_ids, added))
    }

    fn build_by_add(&self, added: &[PathBuf]) -> Result<HashSet<ModuleId>> {
        let files = added
            .iter()
            .map(|path| {
                crate::ast::file::File::new(
                    path.to_string_lossy().to_string(),
                    self.context.clone(),
                )
            })
            .collect();
        self.build(files)
    }

    fn build_by_remove(&self, removed: Vec<PathBuf>) -> (HashSet<ModuleId>, HashSet<ModuleId>) {
        let mut module_graph = self.context.module_graph.write().unwrap();
        let mut removed_module_ids = HashSet::new();
        let mut affected_module_ids = HashSet::new();
        for path in removed {
            let module_id = ModuleId::from_path(path);
            let dependants = module_graph.dependant_module_ids(&module_id);
            module_graph.remove_module_and_deps(&module_id);
            affected_module_ids.extend(dependants);
            removed_module_ids.insert(module_id);
        }
        (removed_module_ids, affected_module_ids)
    }
}

#[derive(Debug)]
pub struct Diff {
    added: HashSet<ModuleId>,
    removed: HashSet<ModuleId>,
    modified: HashSet<ModuleId>,
}

impl Diff {
    fn dependence_changed(
        &self,
        module_id: &ModuleId,
        module_graph: &ModuleGraph,
        new_dependencies: &[(ModuleId, Dependency)],
    ) -> bool {
        if !self.added.is_empty() {
            return true;
        }

        if !self.removed.is_empty() {
            return true;
        }

        let new_deps: HashMap<&ModuleId, &ResolveType> = new_dependencies
            .iter()
            .map(|(module_id, dep)| (module_id, &dep.resolve_type))
            .collect();

        let original: HashMap<&ModuleId, &ResolveType> = module_graph
            .get_dependencies(module_id)
            .into_iter()
            .map(|(module_id, dep)| (module_id, &dep.resolve_type))
            .collect();

        !new_deps.eq(&original)
    }
}

// 比较两个依赖列表的差异
// 未变化的模块算作 modified，因为依赖数据必然发生了变化；eg: order，span
fn diff(origin: &[(ModuleId, Dependency)], new_deps: &[(ModuleId, Dependency)]) -> Diff {
    let origin_module_ids = origin
        .iter()
        .map(|(module_id, _dep)| module_id.clone())
        .collect::<HashSet<_>>();
    let target_module_ids = new_deps
        .iter()
        .map(|(module_id, _dep)| module_id.clone())
        .collect::<HashSet<_>>();

    let removed = origin_module_ids
        .difference(&target_module_ids)
        .cloned()
        .collect::<HashSet<_>>();

    let added = target_module_ids
        .difference(&origin_module_ids)
        .cloned()
        .collect::<HashSet<_>>();

    let modified = origin_module_ids
        .intersection(&target_module_ids)
        .cloned()
        .collect::<HashSet<_>>();

    Diff {
        added,
        removed,
        modified,
    }
}
