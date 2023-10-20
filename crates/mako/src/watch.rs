use std::path::PathBuf;
use std::sync::mpsc::channel;

use mako_core::notify::event::{AccessKind, CreateKind, DataChange, ModifyKind, RenameMode};
use mako_core::notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::update::UpdateType;

#[derive(Debug)]
pub enum WatchEvent {
    Added(Vec<PathBuf>),
    Modified(Vec<PathBuf>),
    #[allow(dead_code)]
    Removed(Vec<PathBuf>),
}

impl From<WatchEvent> for Vec<(PathBuf, UpdateType)> {
    fn from(event: WatchEvent) -> Self {
        match event {
            WatchEvent::Modified(paths) => paths
                .into_iter()
                .map(|path| (path, UpdateType::Modify))
                .collect(),
            WatchEvent::Removed(paths) => paths
                .into_iter()
                .map(|path| (path, UpdateType::Remove))
                .collect(),
            WatchEvent::Added(paths) => paths
                .into_iter()
                .map(|path| (path, UpdateType::Add))
                .collect(),
        }
    }
}

pub fn watch<T>(root: &PathBuf, mut func: T)
where
    T: FnMut(WatchEvent),
{
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.send(res).unwrap();
        },
        mako_core::notify::Config::default(),
    )
    .unwrap();

    // why comment this?
    // ref: #339
    // watcher.watch(root, RecursiveMode::NonRecursive).unwrap();

    std::fs::read_dir(root).unwrap().for_each(|entry| {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            watcher
                .watch(path.as_path(), RecursiveMode::NonRecursive)
                .unwrap();
        } else {
            // TODO respect to .gitignore sth like that
            let path_str = path.to_string_lossy();
            if path_str.contains("node_modules")
                || path_str.contains(".git")
                || path_str.contains("dist")
                || path_str.contains(".DS_Store")
            {
                return;
            }
            watcher
                .watch(path.as_path(), RecursiveMode::Recursive)
                .unwrap();
        }
    });

    while let Ok(event) = rx.recv().unwrap() {
        match event.kind {
            EventKind::Create(CreateKind::File) => {
                func(crate::watch::WatchEvent::Added(event.paths));
            }
            EventKind::Modify(ModifyKind::Data(DataChange::Content)) => {
                if cfg!(target_os = "macos") {
                    func(crate::watch::WatchEvent::Modified(event.paths));
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
                func(crate::watch::WatchEvent::Removed(event.paths));
            }
            EventKind::Remove(_) => {
                println!("removed");
                func(crate::watch::WatchEvent::Removed(event.paths));
            }
            EventKind::Access(AccessKind::Close(_)) => {
                if cfg!(target_os = "linux") {
                    func(crate::watch::WatchEvent::Modified(event.paths));
                }
            }
            _ => {}
        }
    }
}
