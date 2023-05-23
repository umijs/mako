use std::collections::{HashSet, VecDeque};
use std::fmt::Error;
use std::sync::Arc;

use nodejs_resolver::Resolver;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, info};

use crate::build::build::{BuildParam, Task};
use crate::build::load::{load, LoadParam};
use crate::build::parse::{parse, ParseParam};
use crate::build::transform::transform::{transform, TransformParam};
use crate::compiler::Compiler;
use crate::context::Context as MakoContext;
use crate::module::{Module, ModuleId};
use crate::module_graph::Dependency;

pub struct BfsIterator {
    queue: VecDeque<Task>,
    visited: HashSet<String>,
    sender: UnboundedSender<ModuleVisit>,
    receiver: UnboundedReceiver<ModuleVisit>,
    running: u32,
    context: Arc<MakoContext>,
}

impl BfsIterator {
    pub fn new(ctx: Arc<MakoContext>, start: Task) -> Self {
        let (result_sender, result_receiver) =
            tokio::sync::mpsc::unbounded_channel::<ModuleVisit>();
        let mut queue = VecDeque::new();
        queue.push_back(start);

        Self {
            queue,
            visited: HashSet::new(),
            sender: result_sender,
            receiver: result_receiver,
            running: 0,
            context: ctx,
        }
    }
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Iterator for BfsIterator {
    type Item = Result<ModuleVisit, TryRecvError>;

    fn next(&mut self) -> Option<Self::Item> {
        // put all task in tokio threads
        while let Some(edge) = self.queue.pop_front() {
            let sender = self.sender.clone();

            // Spawn a new task to process this edge's children
            let ctx = self.context.clone();
            let res = ctx.resolver.clone();
            self.running += 1;
            self.visited.insert(edge.path.to_string());
            tokio::spawn({
                async move {
                    let result =
                        Compiler::visit_module(ctx, edge, &BuildParam { files: None }, res);
                    sender.send(result).expect("Failed to send build result");
                }
            });
        }

        // return when tokio task returns a non-empty tasks
        loop {
            match self.receiver.try_recv() {
                Ok(visit) => {
                    self.running -= 1;

                    for d in visit.visit_results.iter() {
                        if let VisitResult::Imported(dep) = d {
                            let task = Task {
                                path: dep.path.clone(),
                                parent_module_id: Some(visit.module_id.clone()),
                                parent_dependency: Some(dep.by.clone()),
                            };

                            if !self.visited.contains(&task.path) && !self.queue.contains(&task) {
                                self.queue.push_back(task);
                            }
                        } else if let VisitResult::Externalized(ed) = d {
                            debug!("MyDependence::Externalized {}", ed.path);
                        }
                    }
                    return Some(Ok(visit));
                }
                Err(TryRecvError::Empty) => {
                    if self.running == 0 {
                        return None;
                    }
                    continue;
                }
                Err(TryRecvError::Disconnected) => {
                    return Some(Err(TryRecvError::Disconnected));
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ModuleVisit {
    pub module_id: ModuleId,
    pub current: crate::module::ModuleInfo,
    pub visit_results: Vec<VisitResult>,
}
#[derive(Debug)]
pub struct ImportedDependence {
    pub by: Dependency,
    pub path: String,
    #[allow(dead_code)]
    pub parent: ModuleId,
}
#[derive(Debug)]
pub struct ExternalizedDependence {
    pub path: String,
    pub external_name: String,
    pub by: Dependency,
}

#[derive(Debug)]
pub enum VisitResult {
    Imported(ImportedDependence),
    Externalized(ExternalizedDependence),
}

#[derive(Default, Debug)]
pub struct WalkResult {
    pub added: HashSet<ModuleId>,
    pub removed: HashSet<ModuleId>,
}
impl Compiler {
    pub fn build_module_graph(&mut self, _build_param: &BuildParam) -> Result<(), Error> {
        let cwd = &self.context.config.root;
        let entry_point = cwd
            .join(crate::config::get_first_entry_value(&self.context.config.entry).unwrap())
            .to_string_lossy()
            .to_string();

        self.context
            .module_graph
            .write()
            .unwrap()
            .mark_entry_module(&ModuleId::new(&entry_point));

        self.walk(Task {
            parent_dependency: None,
            parent_module_id: None,
            path: entry_point,
        })?;
        Ok(())
    }

    pub fn walk(&self, from: Task) -> Result<WalkResult, Error> {
        let bfs_visit = BfsIterator::new(self.context.clone(), from);
        let mut walk_result = WalkResult {
            ..Default::default()
        };
        for v in bfs_visit {
            match v {
                Ok(visit) => {
                    let mut need_removed_module_id: Vec<ModuleId> = vec![];
                    let mut added_deps: HashSet<Dependency> = HashSet::new();
                    let mut remove_deps: HashSet<Dependency> = HashSet::new();
                    let from_module_id = visit.module_id;
                    {
                        let mut module_graph_w = self.context.module_graph.write().unwrap();
                        let module = module_graph_w.get_or_add_module(&from_module_id);
                        module.add_info(visit.current.clone());
                        let left_dependencies = module_graph_w.get_dependencies(&from_module_id);
                        let visit = diff_visit(&left_dependencies, &visit.visit_results);
                        added_deps.extend(visit.0);
                        remove_deps.extend(visit.1);
                        {
                            for remove in &remove_deps {
                                let mut to_module_id = None;
                                for (to_id, dep) in &left_dependencies {
                                    if **dep == *remove {
                                        to_module_id = Some(*to_id);
                                        break;
                                    }
                                }
                                let to_module_id = to_module_id.unwrap();
                                need_removed_module_id.push(to_module_id.clone());
                                walk_result.removed.insert(to_module_id.clone());
                            }
                        }
                    }
                    // 清理已经移除的依赖
                    {
                        let mut module_graph_w = self.context.module_graph.write().unwrap();
                        for module_id in need_removed_module_id {
                            module_graph_w.remove_dependency(&from_module_id, &module_id)?;
                            module_graph_w.remove_module(&module_id);
                        }
                    }

                    for dep_edge in &visit.visit_results {
                        match dep_edge {
                            VisitResult::Imported(dep) => {
                                let to_module_id = ModuleId::new(&dep.path.clone());

                                if added_deps.contains(&(dep.by.clone())) {
                                    walk_result.added.insert(to_module_id.clone());
                                }

                                {
                                    let mut module_graph_w =
                                        self.context.module_graph.write().unwrap();
                                    module_graph_w.get_or_add_module(&to_module_id);
                                    module_graph_w.add_dependency(
                                        &from_module_id,
                                        &to_module_id,
                                        dep.by.clone(),
                                    );
                                }
                            }
                            VisitResult::Externalized(dep) => {
                                let to_module_id = ModuleId::new(&dep.path.clone());
                                let mut module = Module::new(to_module_id.clone());
                                module.add_info(crate::module::ModuleInfo {
                                    path: dep.path.clone(),
                                    external_name: Some(dep.external_name.clone()),
                                    is_external: true,
                                    is_entry: false,
                                    original_cm: None,
                                    original_ast: Compiler::build_external(
                                        &dep.external_name.clone(),
                                    ),
                                });
                                {
                                    let mut module_graph_w =
                                        self.context.module_graph.write().unwrap();
                                    module_graph_w.add_module(module);
                                    module_graph_w.add_dependency(
                                        &from_module_id,
                                        &to_module_id,
                                        dep.by.clone(),
                                    )
                                }
                            }
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("Disconnected");
                }
                _ => {}
            }
        }

        Ok(walk_result)
    }

    pub(crate) fn visit_module(
        context: Arc<MakoContext>,
        task: Task,
        build_param: &BuildParam,
        resolver: Arc<Resolver>,
    ) -> ModuleVisit {
        let path_str = task.path.as_str();
        let module_id = ModuleId::new(path_str);

        // load
        let load_param = LoadParam {
            path: path_str,
            files: build_param.files.as_ref(),
        };
        let load_result = load(&load_param, &context);

        // parse
        let parse_param = ParseParam {
            path: path_str,
            content: load_result.content,
            content_type: load_result.content_type,
        };
        let parse_result = parse(&parse_param, &context);

        // transform
        let transform_param = TransformParam {
            path: path_str,
            ast: &parse_result.ast,
            cm: &parse_result.cm,
        };
        let transform_result = transform(&transform_param, &context);

        // add module info
        let current = crate::module::ModuleInfo {
            path: task.path.clone(),
            is_external: false,
            external_name: None,
            is_entry: false,
            original_cm: Some(parse_result.cm),
            original_ast: transform_result.ast.clone(),
        };

        // analyze deps
        let analyze_deps_param = crate::build::analyze_deps::AnalyzeDepsParam {
            path: path_str,
            ast: &parse_result.ast,
            transform_ast: &transform_result.ast,
        };
        let analyze_deps_result =
            crate::build::analyze_deps::analyze_deps(&analyze_deps_param, &context);
        // resolve

        let mut my_dependencies = vec![];
        for d in &analyze_deps_result.dependencies {
            let resolve_param = crate::build::resolve::ResolveParam {
                path: path_str,
                dependency: &d.source,
                files: None,
            };
            let resolve_result =
                crate::build::resolve::resolve(&resolve_param, &context, &resolver);
            debug!("resolve {} -> {}", &d.source, resolve_result.path);
            info!(
                "from {} to {} (external={})",
                path_str, resolve_result.path, resolve_result.is_external
            );
            if resolve_result.is_external {
                let external_name = resolve_result.external_name.unwrap();

                my_dependencies.push(VisitResult::Externalized(ExternalizedDependence {
                    by: d.clone(),
                    path: resolve_result.path.clone(),
                    external_name: external_name.clone(),
                }));
            } else {
                debug!("put task in {}", resolve_result.path);
                my_dependencies.push(VisitResult::Imported(ImportedDependence {
                    by: d.clone(),
                    path: resolve_result.path.clone(),
                    parent: module_id.clone(),
                }));
            }
        }

        ModuleVisit {
            module_id,
            current,
            visit_results: my_dependencies,
        }
    }
}

/**
 * 对比两颗 dependency 的差别
 */
pub fn diff_visit(
    current: &[(&ModuleId, &Dependency)],
    visit_deps: &[VisitResult],
) -> (HashSet<Dependency>, HashSet<Dependency>) {
    let left: HashSet<&Dependency> = current.iter().map(|(_, dep)| *dep).collect();
    let right: HashSet<&Dependency> = visit_deps
        .iter()
        .map(|dep| match dep {
            VisitResult::Imported(dep) => &dep.by,
            VisitResult::Externalized(dep) => &dep.by,
        })
        .collect();
    let added = right
        .difference(&left)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|dep| (**dep).clone())
        .collect();
    let removed = left
        .difference(&right)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|dep| (**dep).clone())
        .collect();
    (added, removed)
}
