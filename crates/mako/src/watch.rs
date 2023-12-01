use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use mako_core::anyhow;
use mako_core::notify::{self, EventKind, Watcher};
use mako_core::notify_debouncer_full::DebouncedEvent;
use mako_core::tracing::debug;

#[derive(Debug)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub event_type: WatchEventType,
}

#[derive(Debug, Clone)]
pub enum WatchEventType {
    Added,
    Modified,
    Removed,
}

pub struct Watch {
    pub root: PathBuf,
    pub delay: u64,
    pub tx: Sender<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
}

impl Watch {
    // pub fn watch(root: &PathBuf, watcher: &mut notify::RecommendedWatcher) -> anyhow::Result<()> {
    pub fn watch(root: &PathBuf, watcher: &mut notify::KqueueWatcher) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn should_ignore_watch(path: &Path) -> bool {
        let path = path.to_string_lossy();
        let ignore_list = [".git", "node_modules", ".DS_Store", "dist"];
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
    pub fn normalize_events(events: Vec<DebouncedEvent>) -> Vec<WatchEvent> {
        // events: { event: { kind, paths: string[] }, time: { tv_sec, tv_nsec } }[]
        // collect paths
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
        debug!("paths: {:?}", paths);

        let mut watch_events = vec![];
        paths.iter().for_each(|path| {
            watch_events.push(WatchEvent {
                path: path.clone(),
                event_type: if create_paths.get(path).is_some() {
                    WatchEventType::Added
                } else if path.exists() {
                    // Added or Modified?
                    WatchEventType::Modified
                } else {
                    WatchEventType::Removed
                },
            });
        });
        watch_events
    }
}
