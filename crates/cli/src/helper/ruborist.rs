use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use std::sync::Mutex;
use tokio::sync::Semaphore;

use crate::helper::workspace::find_workspaces;
use crate::util::config::get_legacy_peer_deps;
use crate::util::logger::{
    finish_progress_bar, log_progress, log_verbose, start_progress_bar, PROGRESS_BAR,
};
use crate::util::node::{Edge, EdgeType, Node};
use crate::util::registry::{load_cache, resolve, store_cache, ResolvedPackage};
use crate::util::semver::matches;

pub struct Ruborist {
    path: PathBuf,
    pub ideal_tree: Option<Arc<Node>>,
}

use once_cell::sync::Lazy;

// concurrency limit default to 100
static CONCURRENCY_LIMITER: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(100)));

async fn build_deps(root: Arc<Node>) -> io::Result<()> {
    let legacy_peer_deps = get_legacy_peer_deps();
    log_verbose(&format!(
        "going to build deps for {}, legacy_peer_deps: {}",
        root.name, legacy_peer_deps
    ));
    let current_level = Arc::new(Mutex::new(vec![root.clone()]));

    while !current_level.lock().unwrap().is_empty() {
        let next_level = Arc::new(Mutex::new(Vec::new()));
        let nodes = current_level.lock().unwrap().clone();
        let mut level_tasks = Vec::new();

        for node in nodes {
            let edges = node.edges_out.write().unwrap();
            let total_deps = edges.len();
            PROGRESS_BAR.inc_length(total_deps as u64);
            log_progress(&format!("Resolving dependencies for {}", node.name));

            let mut tasks = Vec::new();

            for edge in edges.iter() {
                let edge = edge.clone();
                let next_level = next_level.clone();

                tasks.push(async move {
                    let _permit = CONCURRENCY_LIMITER.acquire().await.unwrap();

                    if *edge.valid.read().unwrap() {
                        log_verbose(&format!("deps {}@{} already resolved", edge.name, edge.spec));
                        // when the edge.to is workspace, add it to next_level
                        if let Some(new_node) = edge.to.write().unwrap().as_ref().cloned() {
                            if new_node.is_workspace {
                                next_level.lock().unwrap().push(new_node);
                            }
                        }
                        // processed
                        return Ok(());
                    }


                    log_verbose(&format!("going to build deps {}@{} from [{}]", edge.name, edge.spec, edge.from.name));

                    match find_compatible_node(&edge.from, &edge.name, &edge.spec) {
                        FindResult::Reuse(existing_node) => {
                            log_verbose(&format!(
                                "resolved deps {}@{} => {} (reuse)",
                                edge.name, &edge.spec, existing_node.version
                            ));
                            {
                                let mut to = edge.to.write().unwrap();
                                *to = Some(existing_node.clone());
                                let mut valid = edge.valid.write().unwrap();
                                *valid = true;
                            }

                            // update node type by edges
                            existing_node.add_invoke(&edge);
                            existing_node.update_type();
                        }
                        FindResult::Conflict(conflict_node) => {
                            let resolved = resolve(&edge.name, &edge.spec).await?;
                            PROGRESS_BAR.inc(1);
                            log_progress(&format!(
                                "resolved deps {}@{} => {} (conflict), need to fork, conflict_node: {:?}",
                                edge.name, &edge.spec, resolved.version, conflict_node.name
                            ));
                            log_verbose(&format!(
                                "resolved deps {}@{} => {} (conflict), need to fork, conflict_node: {:?}",
                                edge.name, &edge.spec, resolved.version, conflict_node.name
                            ));
                            // process conflict node
                            let install_parent = conflict_node;
                            let new_node = place_deps(edge.name.clone(), resolved, &install_parent)?;

                            {
                                let mut parent = new_node.parent.write().unwrap();
                                *parent = Some(install_parent.clone());
                                let mut children = install_parent.children.write().unwrap();
                                children.push(new_node.clone());

                                let mut to = edge.to.write().unwrap();
                                *to = Some(new_node.clone());
                                let mut valid = edge.valid.write().unwrap();
                                *valid = true;
                                // update node type
                                new_node.add_invoke(&edge);
                                new_node.update_type();
                            }

                            let dep_types = if legacy_peer_deps {
                                vec![
                                    ("dependencies", EdgeType::Prod),
                                    ("optionalDependencies", EdgeType::Optional),
                                ]
                            } else {
                                vec![
                                    ("dependencies", EdgeType::Prod),
                                    ("peerDependencies", EdgeType::Peer),
                                    ("optionalDependencies", EdgeType::Optional),
                                ]
                            };

                            for (field, edge_type) in dep_types {
                                if let Some(deps) = new_node.package.get(field) {
                                    if let Some(deps) = deps.as_object() {
                                        for (name, version) in deps {
                                            let version_spec = version.as_str().unwrap_or("").to_string();
                                            let dep_edge = Edge::new(new_node.clone(), edge_type.clone(), name.clone(), version_spec);
                                            log_verbose(&format!(
                                                "add edge {}@{} for {}",
                                                name, version, new_node.name
                                            ));
                                            new_node.add_edge(dep_edge).await;
                                        }
                                    }
                                }
                            }

                            next_level.lock().unwrap().push(new_node);
                        }
                        FindResult::New(install_location) => {
                            let resolved = resolve(&edge.name, &edge.spec).await?;
                            PROGRESS_BAR.inc(1);
                            log_progress(&format!(
                                "resolved deps {}@{} => {} (new)",
                                edge.name, &edge.spec, resolved.version
                            ));
                            log_verbose(&format!(
                                "resolved deps {}@{} => {} (new)",
                                edge.name, &edge.spec, resolved.version
                            ));
                            let new_node = place_deps(edge.name.clone(), resolved, &install_location)?;
                            let root_node = install_location.clone();

                            {
                                let mut parent = new_node.parent.write().unwrap();
                                *parent = Some(root_node.clone());
                            }
                            {
                                let mut children = root_node.children.write().unwrap();
                                children.push(new_node.clone());
                            }
                            {
                                let mut to = edge.to.write().unwrap();
                                *to = Some(new_node.clone());
                                let mut valid = edge.valid.write().unwrap();
                                *valid = true;
                                // update node type
                                new_node.add_invoke(&edge);
                                new_node.update_type();
                            }

                            add_dependency_edge(&new_node, "dependencies", EdgeType::Prod).await;

                            if !legacy_peer_deps {
                                add_dependency_edge(&new_node, "peerDependencies", EdgeType::Peer).await;
                            }

                            add_dependency_edge(&new_node, "optionalDependencies", EdgeType::Optional).await;

                            next_level.lock().unwrap().push(new_node);
                        }
                    }
                    Ok::<_, io::Error>(())
                });
            }
            level_tasks.push(futures::future::try_join_all(tasks));
        }

        // waiting for all tasks in this level to finish
        futures::future::try_join_all(level_tasks).await?;

        // continue to next level
        *current_level.lock().unwrap() = next_level.lock().unwrap().clone();
    }

    Ok(())
}

// create a new node under parent
fn place_deps(name: String, pkg: ResolvedPackage, parent: &Arc<Node>) -> io::Result<Arc<Node>> {
    let new_node = Node::new(name, parent.path.clone(), pkg.manifest);

    log_verbose(&format!(
        "\nInstalling {}@{} under parent chain: {}",
        new_node.name, new_node.version, parent
    ));
    // log_verbose(&print_parent_chain(parent));
    log_verbose("");

    Ok(new_node)
}

#[derive(Debug)]
pub enum FindResult {
    Reuse(Arc<Node>),    // can resue
    Conflict(Arc<Node>), // conflict, return parent node
    New(Arc<Node>),      // need to install under root node
}

fn find_compatible_node(from: &Arc<Node>, name: &str, version_spec: &str) -> FindResult {
    fn find_in_node(
        node: &Arc<Node>,
        name: &str,
        version_spec: &str,
        current: &Arc<Node>,
    ) -> FindResult {
        let children = node.children.read().unwrap();

        for child in children.iter() {
            if child.name == name {
                if matches(version_spec, &child.version) {
                    log_verbose(&format!(
                        "found existing deps {}@{} got {}",
                        name, version_spec, child.version
                    ));
                    return FindResult::Reuse(child.clone());
                } else {
                    log_verbose(&format!(
                        "found conflict deps {}@{} got {}",
                        name, version_spec, child.version
                    ));
                    return FindResult::Conflict(current.clone());
                }
            }
        }

        // find in parent
        if let Some(parent) = node.parent.read().unwrap().as_ref() {
            find_in_node(parent, name, version_spec, current)
        } else {
            // not found, return new
            FindResult::New(node.clone())
        }
    }

    if let Some(parent) = from.parent.read().unwrap().as_ref() {
        find_in_node(parent, name, version_spec, from)
    } else {
        find_in_node(from, name, version_spec, from)
    }
}

impl Ruborist {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            path: path.into(),
            ideal_tree: None,
        }
    }

    async fn init_tree(&mut self) -> io::Result<Arc<Node>> {
        // load package.json
        let pkg_path = self.path.join("package.json");
        let pkg_content = std::fs::read_to_string(pkg_path)?;
        let pkg: Value = serde_json::from_str(&pkg_content)?;

        // create root node
        let root = Node::new_root(
            pkg["name"].as_str().unwrap_or("root").to_string(),
            self.path.clone(),
            pkg.clone(),
        );
        log_verbose(&format!("root node: {:?}", root));

        self.init_workspaces(root.clone()).await?;

        // collect deps type
        let legacy_peer_deps = get_legacy_peer_deps();
        let dep_types = if legacy_peer_deps {
            vec![
                ("dependencies", EdgeType::Prod),
                ("devDependencies", EdgeType::Dev),
                ("optionalDependencies", EdgeType::Optional),
            ]
        } else {
            vec![
                ("dependencies", EdgeType::Prod),
                ("devDependencies", EdgeType::Dev),
                ("peerDependencies", EdgeType::Peer),
                ("optionalDependencies", EdgeType::Optional),
            ]
        };

        // process deps in root
        for (field, dep_type) in dep_types {
            if let Some(deps) = pkg.get(field) {
                if let Some(deps) = deps.as_object() {
                    for (name, version) in deps {
                        log_verbose(&format!("{}: {}", name, version));
                        let version_spec = version.as_str().unwrap_or("").to_string();

                        // create edge
                        let edge = Edge::new(
                            root.clone(), // need clone Arc<Node>
                            dep_type.clone(),
                            name.clone(),
                            version_spec,
                        );

                        log_verbose(&format!("add edge {}@{}", edge.name, edge.spec));
                        root.add_edge(edge).await;
                    }
                }
            }
        }

        Ok(root)
    }

    pub async fn init_workspaces(&mut self, root: Arc<Node>) -> io::Result<()> {
        let workspaces = find_workspaces(&self.path).await?;

        // Process each workspace
        for (name, path, pkg) in workspaces {
            let version = pkg["version"].as_str().unwrap_or("").to_string();

            // Create workspace node
            let workspace_node = Node::new_workspace(name.clone(), path, pkg.clone());

            // Create link node
            let link_node = Node::new_link(name.clone(), workspace_node.clone());

            // Create dependency edge
            let dep_edge = Edge::new(root.clone(), EdgeType::Prod, name.clone(), version);

            // Set target node and validity for dependency edge
            {
                let mut valid = dep_edge.valid.write().unwrap();
                *valid = true;

                let mut to = dep_edge.to.write().unwrap();
                *to = Some(workspace_node.clone());
            }

            // Update parent relationships
            {
                let mut parent = workspace_node.parent.write().unwrap();
                *parent = Some(root.clone());
            }
            {
                let mut parent = link_node.parent.write().unwrap();
                *parent = Some(root.clone());
            }
            {
                let mut children = root.children.write().unwrap();
                children.push(workspace_node.clone());
                children.push(link_node);
            }

            // Add dependency edge
            root.add_edge(dep_edge).await;

            log_verbose(&format!(
                "Added workspace: {} {:?}",
                name, workspace_node.path
            ));

            // Process workspace dependencies
            let legacy_peer_deps = get_legacy_peer_deps();
            let dep_types = if legacy_peer_deps {
                vec![
                    ("devDependencies", EdgeType::Dev),
                    ("dependencies", EdgeType::Prod),
                    ("optionalDependencies", EdgeType::Optional),
                ]
            } else {
                vec![
                    ("devDependencies", EdgeType::Dev),
                    ("dependencies", EdgeType::Prod),
                    ("peerDependencies", EdgeType::Peer),
                    ("optionalDependencies", EdgeType::Optional),
                ]
            };

            for (field, edge_type) in dep_types {
                if let Some(deps) = workspace_node.package.get(field) {
                    if let Some(deps) = deps.as_object() {
                        for (name, version) in deps {
                            let version_spec = version.as_str().unwrap_or("").to_string();
                            let dep_edge = Edge::new(
                                workspace_node.clone(),
                                edge_type.clone(),
                                name.clone(),
                                version_spec,
                            );
                            log_verbose(&format!(
                                "add edge {}@{} for {}",
                                name, version, workspace_node.name
                            ));
                            workspace_node.add_edge(dep_edge).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn build_ideal_tree(&mut self) -> io::Result<()> {
        load_cache("./node_modules/.utoo-manifest.json").await?;
        let root = self.init_tree().await?;

        start_progress_bar();
        build_deps(root.clone()).await?;

        let res = self.get_dup_deps(root.clone());
        for dup_node in res {
            self.replace_deps(dup_node).await?;
        }

        store_cache("./node_modules/.utoo-manifest.json").await?;
        finish_progress_bar("Building dependency tree complete.");
        self.ideal_tree = Some(root);
        Ok(())
    }

    pub async fn build_workspace_tree(&mut self) -> io::Result<()> {
        let root = self.init_tree().await?;

        start_progress_bar();

        // init_tree has already loaded all workspace
        let children = root.children.read().unwrap();

        // build a map of workspace nodes
        let mut workspace_map = HashMap::new();
        for workspace in children.iter() {
            if workspace.is_workspace {
                workspace_map.insert(workspace.name.clone(), workspace.clone());
            }
        }

        // find the deps between workspace
        for workspace in children.iter() {
            if workspace.is_link {
                continue;
            }
            PROGRESS_BAR.inc_length(1);
            let edges = workspace.edges_out.read().unwrap();
            for edge in edges.iter() {
                if let Some(dep_workspace) = workspace_map.get(&edge.name) {
                    // find edges_out for workspace
                    let mut to = edge.to.write().unwrap();
                    *to = Some(dep_workspace.clone());
                    let mut valid = edge.valid.write().unwrap();
                    *valid = true;

                    log_verbose(&format!(
                        "Workspace dependency: {} -> {}",
                        workspace.name, dep_workspace.name
                    ));
                }
            }
            PROGRESS_BAR.inc(1);
        }

        finish_progress_bar("Building workspace dependency tree complete.");
        self.ideal_tree = Some(root.clone());
        Ok(())
    }

    pub fn get_dup_deps(&self, root: Arc<Node>) -> Vec<Arc<Node>> {
        let mut duplicates = Vec::new();

        fn process_node(node: &Arc<Node>, duplicates: &mut Vec<Arc<Node>>) {
            let children = node.children.read().unwrap();
            let mut name_map: HashMap<String, Vec<Arc<Node>>> = HashMap::new();

            // find duplicate deps
            for child in children.iter() {
                if child.is_workspace {
                    continue;
                }
                name_map
                    .entry(child.name.clone())
                    .or_insert_with(Vec::new)
                    .push(child.clone());
            }

            // hanlde dup node
            for (_, nodes) in name_map {
                if nodes.len() > 1 {
                    // find max edges_in node to save the cost
                    let mut max_edges = 0;
                    let mut primary_node = None;

                    for node in &nodes {
                        let edges_count = node.edges_in.read().unwrap().len();
                        if edges_count > max_edges {
                            max_edges = edges_count;
                            primary_node = Some(node.clone());
                        }
                    }

                    // add to duplicates
                    if let Some(primary) = primary_node {
                        for node in nodes {
                            if !Arc::ptr_eq(&node, &primary) {
                                duplicates.push(node);
                            }
                        }
                    }
                }
            }

            for child in children.iter() {
                process_node(child, duplicates);
            }
        }

        process_node(&root, &mut duplicates);
        duplicates
    }

    pub async fn replace_deps(&self, node: Arc<Node>) -> io::Result<()> {
        log_verbose(&format!("going to replace node {}", node));
        // 1. remove from parent node
        if let Some(parent) = node.parent.read().unwrap().as_ref() {
            let mut parent_children = parent.children.write().unwrap();
            parent_children.retain(|child| !Arc::ptr_eq(child, &node));
        }

        // 2. clean edges_out
        {
            let mut edges_out = node.edges_out.write().unwrap();
            edges_out.clear();
        }

        // 3. clean edges_in
        let edges_from = {
            let edges_in = node.edges_in.read().unwrap();
            edges_in
                .iter()
                .map(|edge| {
                    let mut valid = edge.valid.write().unwrap();
                    *valid = false;

                    let mut to = edge.to.write().unwrap();
                    *to = None;

                    edge.from.clone()
                })
                .collect::<Vec<_>>()
        };

        // 4. rebuild deps
        for from_node in edges_from {
            build_deps(from_node).await?;
        }

        Ok(())
    }
}

async fn add_dependency_edge(node: &Arc<Node>, field: &str, edge_type: EdgeType) {
    if let Some(deps) = node.package.get(field) {
        if let Some(deps) = deps.as_object() {
            for (name, version) in deps {
                let version_spec = version.as_str().unwrap_or("").to_string();
                let dep_edge =
                    Edge::new(node.clone(), edge_type.clone(), name.clone(), version_spec);
                log_verbose(&format!("add edge {}@{} for {}", name, version, node.name));
                node.add_edge(dep_edge).await;
            }
        }
    }
}
