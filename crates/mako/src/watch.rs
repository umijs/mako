use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mako_core::anyhow::{self, Ok};
use mako_core::notify::{self, EventKind, Watcher as NotifyWatcher};
use mako_core::notify_debouncer_full::DebouncedEvent;

use crate::compiler::Compiler;
use crate::resolve::ResolverResource;

pub struct Watcher<'a> {
    pub watcher: &'a mut dyn NotifyWatcher,
    pub root: &'a PathBuf,
    pub compiler: &'a Compiler,
    pub watched_files: HashSet<PathBuf>,
    pub watched_dirs: HashSet<PathBuf>,
}

impl<'a> Watcher<'a> {
    pub fn new(
        root: &'a PathBuf,
        watcher: &'a mut notify::RecommendedWatcher,
        compiler: &'a Arc<Compiler>,
    ) -> Self {
        Self {
            root,
            watcher,
            compiler,
            watched_dirs: HashSet::new(),
            watched_files: HashSet::new(),
        }
    }

    // pub fn watch(root: &PathBuf, watcher: &mut notify::RecommendedWatcher) -> anyhow::Result<()> {
    pub fn watch(&mut self) -> anyhow::Result<()> {
        self.watch_dir_recursive(
            self.root.into(),
            &[".git", "node_modules", ".DS_Store", "dist", ".node"],
        )?;

        let module_graph = self.compiler.context.module_graph.read().unwrap();
        module_graph
            .modules()
            .iter()
            .try_for_each(|module| -> anyhow::Result<()> {
                if let Some(ResolverResource::Resolved(resource)) = module
                    .info
                    .as_ref()
                    .and_then(|info| info.resolved_resource.as_ref())
                {
                    if let Some(dir) = &resource.0.description {
                        let dir = dir.dir().as_ref();
                        // not in root dir or is root's parent dir
                        if dir.strip_prefix(self.root).is_err()
                            && self.root.clone().strip_prefix(dir).is_err()
                        {
                            self.watch_dir_recursive(
                                dir.into(),
                                &[".git", "node_modules", ".DS_Store", ".node"],
                            )?;
                        }
                    }
                }

                Ok(())
            })?;

        Ok(())
    }

    pub fn refresh_watch(&mut self) -> anyhow::Result<()> {
        self.watch()?;
        Ok(())
    }

    fn watch_dir_recursive(&mut self, path: PathBuf, ignore_list: &[&str]) -> anyhow::Result<()> {
        let items = std::fs::read_dir(path)?;
        items
            .into_iter()
            .try_for_each(|item| -> anyhow::Result<()> {
                let path = item.unwrap().path();
                self.watch_file_or_dir(path, ignore_list)?;
                Ok(())
            })?;
        Ok(())
    }

    fn watch_file_or_dir(&mut self, path: PathBuf, ignore_list: &[&str]) -> anyhow::Result<()> {
        if Self::should_ignore_watch(&path, ignore_list) {
            return Ok(());
        }

        if path.is_file() && !self.watched_files.contains(&path) {
            self.watcher
                .watch(path.as_path(), notify::RecursiveMode::NonRecursive)?;
            self.watched_files.insert(path);
        } else if path.is_dir() && !self.watched_dirs.contains(&path) {
            self.watcher
                .watch(path.as_path(), notify::RecursiveMode::Recursive)?;
            self.watched_dirs.insert(path);
        } else {
            // others like symlink? should be ignore?
        }

        Ok(())
    }

    fn should_ignore_watch(path: &Path, ignore_list: &[&str]) -> bool {
        let path = path.to_string_lossy();
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
