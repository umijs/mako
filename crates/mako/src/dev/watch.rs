use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{self, Ok};
use colored::Colorize;
use notify::{self, EventKind, Watcher as NotifyWatcher};
use notify_debouncer_full::DebouncedEvent;
use regex::Regex;
use tracing::debug;

use crate::compiler::Compiler;
use crate::resolve::ResolverResource;

pub struct Watcher<'a> {
    pub watcher: &'a mut dyn NotifyWatcher,
    pub root: &'a PathBuf,
    pub compiler: &'a Compiler,
    pub watched_files: HashSet<PathBuf>,
    pub watched_dirs: HashSet<PathBuf>,
    node_modules_regexes: Vec<Regex>,
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
            node_modules_regexes: compiler
                .context
                .config
                .watch
                .node_modules_regexes
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect::<Vec<Regex>>(),
        }
    }

    // pub fn watch(root: &PathBuf, watcher: &mut notify::RecommendedWatcher) -> anyhow::Result<()> {
    pub fn watch(&mut self) -> anyhow::Result<()> {
        let t_watch = Instant::now();

        self.watch_dir_recursive(self.root.into(), &self.get_ignore_list(true))?;

        let module_graph = self.compiler.context.module_graph.read().unwrap();
        let mut dirs = HashSet::new();
        module_graph.modules().iter().for_each(|module| {
            if let Some(ResolverResource::Resolved(resource)) = module
                .info
                .as_ref()
                .and_then(|info| info.resolved_resource.as_ref())
            {
                if let Some(dir) = &resource.0.package_json() {
                    let dir = dir.directory();
                    // not in root dir or is root's parent dir
                    if dir.strip_prefix(self.root).is_err() && self.root.strip_prefix(dir).is_err()
                    {
                        dirs.insert(dir);
                    }
                }
                if !self.node_modules_regexes.is_empty() {
                    let file_path = resource.0.path().to_str().unwrap();
                    let is_match = file_path.contains("node_modules")
                        && self
                            .node_modules_regexes
                            .iter()
                            .any(|regex| regex.is_match(file_path));
                    if is_match {
                        let _ = self.watcher.watch(
                            resource.0.path().to_path_buf().as_path(),
                            notify::RecursiveMode::NonRecursive,
                        );
                    }
                }
            }
        });
        dirs.iter().try_for_each(|dir| {
            self.watch_dir_recursive(dir.into(), &self.get_ignore_list(false))?;
            Ok(())
        })?;

        let t_watch_duration = t_watch.elapsed();
        debug!(
            "{}",
            format!(
                "✓ watch in {}",
                format!("{}ms", t_watch_duration.as_millis()).bold()
            )
            .green()
        );

        Ok(())
    }

    fn get_ignore_list(&self, with_output_dir: bool) -> Vec<PathBuf> {
        let mut ignore_list = vec![".git", "node_modules", ".DS_Store", ".node"];
        if with_output_dir {
            ignore_list.push(self.compiler.context.config.output.path.to_str().unwrap());
        }
        ignore_list.extend(
            self.compiler
                .context
                .config
                .watch
                .ignore_paths
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|p| p.as_str()),
        );

        // node_modules of root dictionary and root dictionary's parent dictionaries should be ignored
        // for resolving the issue of "too many files open" in monorepo
        let mut dirs = vec![];
        self.root.ancestors().for_each(|path| {
            ignore_list.iter().for_each(|ignore| {
                let mut path = PathBuf::from(path);
                path.push(ignore);
                dirs.push(path);
            })
        });
        dirs
    }

    fn watch_dir_recursive(
        &mut self,
        path: PathBuf,
        ignore_list: &[PathBuf],
    ) -> anyhow::Result<()> {
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

    fn watch_file_or_dir(&mut self, path: PathBuf, ignore_list: &[PathBuf]) -> anyhow::Result<()> {
        if Self::should_ignore_watch(&path, ignore_list)
            || path.to_string_lossy().contains("node_modules")
        {
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

    fn should_ignore_watch(path: &Path, ignore_list: &[PathBuf]) -> bool {
        let path = path.to_string_lossy();
        ignore_list
            .iter()
            .any(|ignored| path.strip_prefix(ignored.to_str().unwrap()).is_some())
    }

    fn should_ignore_event(path: &Path, kind: &EventKind) -> bool {
        if matches!(
            kind,
            EventKind::Modify(notify::event::ModifyKind::Metadata(_))
        ) {
            return true;
        }
        let ignore_list = [".DS_Store", ".swx", ".swp"];
        // Ignore directory changes, but it should be noted that if the directory is deleted, it cannot be detected at this time
        // TODO: so, should it be put outside, based on whether the module_graph exists this module to judge?
        if path.is_dir() {
            return true;
        }
        let path = path.to_string_lossy();
        ignore_list.iter().any(|ignored| path.ends_with(ignored))
    }

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
