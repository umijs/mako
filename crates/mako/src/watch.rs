use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use mako_core::anyhow::{self, Ok};
use mako_core::notify::{self, EventKind, Watcher};
use mako_core::notify_debouncer_full::DebouncedEvent;

use crate::compiler::Compiler;
use crate::resolve::ResolverResource;

pub struct Watch {
    pub root: PathBuf,
    pub delay: u64,
    pub tx: Sender<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
}

impl Watch {
    // pub fn watch(root: &PathBuf, watcher: &mut notify::RecommendedWatcher) -> anyhow::Result<()> {
    pub fn watch(
        root: &PathBuf,
        watcher: &mut notify::RecommendedWatcher,
        compiler: &Arc<Compiler>,
    ) -> anyhow::Result<()> {
        let items = std::fs::read_dir(root)?;
        items
            .into_iter()
            .try_for_each(|item| -> anyhow::Result<()> {
                let path = item.unwrap().path();
                if Self::should_ignore_watch(&path) {
                    return Ok(());
                }
                if path.is_file() {
                    watcher.watch(path.as_path(), notify::RecursiveMode::NonRecursive)?;
                } else if path.is_dir() {
                    watcher.watch(path.as_path(), notify::RecursiveMode::Recursive)?;
                } else {
                    // others like symlink? should be ignore?
                }
                Ok(())
            })?;

        let module_graph = compiler.context.module_graph.read().unwrap();
        module_graph
            .modules()
            .iter()
            .try_for_each(|module| -> anyhow::Result<()> {
                if let Some(ResolverResource::Resolved(resource)) = module
                    .info
                    .as_ref()
                    .and_then(|info| info.resolved_resource.as_ref())
                {
                    let path = &resource.0.path;
                    watcher.watch(Path::new(&path), notify::RecursiveMode::NonRecursive)?;
                }

                Ok(())
            })?;

        Ok(())
    }

    fn should_ignore_watch(path: &Path) -> bool {
        let path = path.to_string_lossy();
        let ignore_list = [".git", "node_modules", ".DS_Store", "dist", ".node"];
        ignore_list.iter().any(|ignored| path.ends_with(ignored))
    }

    fn should_ignore_event(path: &Path, kind: &EventKind) -> bool {
        if matches!(
            kind,
            EventKind::Modify(notify::event::ModifyKind::Metadata(_))
        ) {
            return true;
        }
        let ignore_list = [".DS_Store", ".swx", ".swp"];
        // 忽略目录变更，但需要注意的是，如果目录被删除，此时无法被检测到
        // TODO: 所以，要不要统一放到外面，基于 module_graph 是否存在此模块来判断？
        if path.is_dir() {
            return true;
        }
        let path = path.to_string_lossy();
        ignore_list.iter().any(|ignored| path.ends_with(ignored))
    }

    // TODO: support notify::Event mode
    pub fn normalize_events(events: Vec<DebouncedEvent>) -> Vec<PathBuf> {
        let mut paths = vec![];
        let mut create_paths = HashMap::new();
        events.iter().for_each(|debounced_event| {
            let kind = &debounced_event.event.kind;
            debounced_event.event.paths.iter().for_each(|path| {
                if Self::should_ignore_event(path, kind) {
                    return;
                }
                paths.push(path.clone());
                if matches!(debounced_event.event.kind, EventKind::Create(_)) {
                    create_paths.insert(path.clone(), true);
                } else {
                    create_paths.remove(path);
                }
            });
        });
        paths.sort();
        paths.dedup();
        paths
    }
}
