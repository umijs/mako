use crate::build::build::BuildParam;
use crate::compiler::Compiler;
use crate::context::Context;
use crate::module::{ModuleId, ModuleInfo2};
use crate::module_graph::Dependency;
use nodejs_resolver::Resolver;
use spliter::Spliterator;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Task2 {
    pub path: String,
    pub is_entry: bool,
    pub parent_module_id: Option<ModuleId>,
    pub parent_dependency: Option<Dependency>,
}

impl PartialEq for Task2 {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

pub struct ModuleBFS {
    queue: VecDeque<Task2>,
    visited: Arc<RwLock<HashSet<String>>>,
    ctx: Arc<Context>,
    build_params: Arc<Mutex<BuildParam>>,
    resolver: Arc<Resolver>,
}

impl Spliterator for ModuleBFS {
    fn split(&mut self) -> Option<Self> {
        self.try_split()
    }
}

impl ModuleBFS {
    pub fn new(entry_point: String, ctx: Arc<Context>, resolver: Arc<Resolver>) -> Self {
        let mut queue = VecDeque::new();
        let visited = Arc::new(RwLock::new(HashSet::new()));

        queue.push_back(Task2 {
            path: entry_point,
            is_entry: true,
            parent_module_id: None,
            parent_dependency: None,
        });

        Self {
            queue,
            visited,
            ctx,
            build_params: Arc::new(Mutex::new(BuildParam { files: None })),
            resolver: Arc::clone(&resolver),
        }
    }

    #[allow(dead_code)]
    pub fn try_split(&mut self) -> Option<Self> {
        if self.queue.len() >= 4 {
            let mid = self.queue.len() / 2;
            let right = self.queue.split_off(mid);
            Some(ModuleBFS {
                queue: right,
                visited: self.visited.clone(),
                ctx: self.ctx.clone(),
                build_params: self.build_params.clone(),
                resolver: self.resolver.clone(),
            })
        } else {
            None
        }
    }
}

pub struct ModuleNode {
    pub(crate) current: ModuleInfo2,
    pub(crate) resolved_module_infos: Vec<ModuleInfo2>,
    pub(crate) dependencies_edges: Vec<ModuleEdge>,
}

pub struct ModuleEdge {
    pub(crate) to: ModuleId,
    pub(crate) dep: Dependency,
}

pub struct ModuleGraphNode {
    pub(crate) current_module_info: ModuleInfo2,
    pub(crate) to_module_infos: Vec<ModuleInfo2>,
    pub(crate) tasks: Vec<Task2>,
}

impl Iterator for ModuleBFS {
    type Item = ModuleNode;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(task) = self.queue.pop_front() {
            let result = Compiler::build_module2(
                self.ctx.clone(),
                &task,
                &self.build_params.lock().unwrap(),
                self.resolver.clone(),
            );

            {
                let mut w = self.visited.write().unwrap();
                w.insert(result.current_module_info.path());
            }

            let edges = result
                .tasks
                .iter()
                .map(|t| ModuleEdge {
                    to: ModuleId::new(t.path.clone().as_str()),
                    dep: t.parent_dependency.as_ref().unwrap().clone(),
                })
                .collect();

            let r = self.visited.read().unwrap();

            let tasks = result
                .tasks
                .into_iter()
                .filter(|task| {
                    if r.contains(&task.path) {
                        return false;
                    }

                    if self.queue.contains(task) {
                        return false;
                    }

                    true
                })
                .collect::<Vec<Task2>>();

            self.queue.extend(tasks);

            Some(ModuleNode {
                current: result.current_module_info,
                resolved_module_infos: result.to_module_infos,
                dependencies_edges: edges,
            })
        } else {
            None
        }
    }
}
