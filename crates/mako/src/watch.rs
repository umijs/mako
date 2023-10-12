use std::path::PathBuf;

use mako_core::notify::event::{CreateKind, DataChange, ModifyKind, RenameMode};
use mako_core::notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use mako_core::tokio::sync::mpsc::channel;

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

pub async fn watch<T>(root: &PathBuf, func: T)
where
    T: FnMut(WatchEvent),
{
    watch_async(root, func).await;
}

pub async fn watch_async<T>(root: &PathBuf, mut func: T)
where
    T: FnMut(WatchEvent),
{
    let (tx, mut rx) = channel(2);
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.blocking_send(res).unwrap();
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

    while let Some(res) = rx.recv().await {
        match res {
            Ok(event) => match event.kind {
                EventKind::Any => {}
                EventKind::Access(_) => {}
                EventKind::Create(CreateKind::File) => {
                    func(crate::watch::WatchEvent::Added(event.paths));
                }
                EventKind::Create(_) => {}

                EventKind::Modify(ModifyKind::Data(DataChange::Any)) => {
                    func(crate::watch::WatchEvent::Modified(event.paths));
                }
                EventKind::Modify(ModifyKind::Name(RenameMode::Any)) => {
                    func(crate::watch::WatchEvent::Removed(event.paths));
                }
                EventKind::Modify(_) => {}
                EventKind::Remove(_) => {
                    func(crate::watch::WatchEvent::Removed(event.paths));
                }
                EventKind::Other => {}
            },
            Err(e) => {
                println!("watch error: {:?}", e);
            }
        }
    }
}
