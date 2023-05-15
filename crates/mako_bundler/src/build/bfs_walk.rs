use std::collections::{HashSet, VecDeque};
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

struct BfsIterator {
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

                    for d in visit.dependencies.iter() {
                        if let MyDependence::Imported(dep) = d {
                            let task = Task {
                                path: dep.path.clone(),
                                parent_module_id: Some(visit.module_id.clone()),
                                parent_dependency: Some(dep.by.clone()),
                            };

                            if !self.visited.contains(&task.path) && !self.queue.contains(&task) {
                                self.queue.push_back(task);
                            }
                        } else if let MyDependence::Externalized(ed) = d {
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
    module_id: ModuleId,
    current: crate::module::ModuleInfo,
    dependencies: Vec<MyDependence>,
}
#[derive(Debug)]
struct ImportedDependence {
    pub by: Dependency,
    pub path: String,
    #[allow(dead_code)]
    pub parent: ModuleId,
}
#[derive(Debug)]
struct ExternalizedDependence {
    path: String,
    external_name: String,
    by: Dependency,
}

#[derive(Debug)]
enum MyDependence {
    Imported(ImportedDependence),
    Externalized(ExternalizedDependence),
}

impl Compiler {
    pub fn build_module_graph(&mut self, _build_param: &BuildParam) {
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
        });

        self.grouping_chunks();
    }

    pub fn walk(&self, from: Task) {
        let bfs_visit = BfsIterator::new(self.context.clone(), from);
        let mut module_graph = self.context.module_graph.write().unwrap();
        for v in bfs_visit {
            match v {
                Ok(visit) => {
                    let module = module_graph.get_or_add_module(&visit.module_id);
                    module.add_info(visit.current.clone());

                    let from_module_id = &visit.module_id;

                    for dep_edge in visit.dependencies {
                        match dep_edge {
                            MyDependence::Imported(dep) => {
                                let to_module_id = ModuleId::new(&dep.path.clone());
                                module_graph.get_or_add_module(&to_module_id);
                                module_graph.add_dependency(
                                    from_module_id,
                                    &to_module_id,
                                    dep.by.clone(),
                                );
                            }
                            MyDependence::Externalized(dep) => {
                                let to_module_id = ModuleId::new(&dep.path.clone());

                                let mut module = Module::new(to_module_id.clone());
                                module.add_info(crate::module::ModuleInfo {
                                    path: dep.path.clone(),
                                    external_name: Some(dep.external_name.clone()),
                                    is_external: true,
                                    is_entry: false,
                                    original_cm: None,
                                    original_ast: crate::module::ModuleAst::None,
                                });

                                module_graph.add_module(module);
                                module_graph.add_dependency(
                                    from_module_id,
                                    &to_module_id,
                                    dep.by.clone(),
                                )
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

        {
            // let mut module_graph_w = context.module_graph.write().unwrap();
            // let module = module_graph_w.get_module_mut(&module_id).unwrap();
            // module.add_info(info);
        }

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

                my_dependencies.push(MyDependence::Externalized(ExternalizedDependence {
                    by: d.clone(),
                    path: resolve_result.path.clone(),
                    external_name: external_name.clone(),
                }));
            } else {
                debug!("put task in {}", resolve_result.path);
                my_dependencies.push(MyDependence::Imported(ImportedDependence {
                    by: d.clone(),
                    path: resolve_result.path.clone(),
                    parent: module_id.clone(),
                }));
            }
        }

        ModuleVisit {
            module_id,
            current,
            dependencies: my_dependencies,
        }
    }
}
