use std::path::PathBuf;

use futures::channel::mpsc::channel;
use futures::{SinkExt, StreamExt};
use notify::event::{CreateKind, DataChange, ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

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

pub fn watch<T>(root: &PathBuf, func: T)
where
    T: FnMut(WatchEvent),
{
    futures::executor::block_on(async {
        watch_async(root, func).await;
    });
}

pub async fn watch_async<T>(root: &PathBuf, mut func: T)
where
    T: FnMut(WatchEvent),
{
    let (mut tx, mut rx) = channel(2);
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        notify::Config::default(),
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

    while let Some(res) = rx.next().await {
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
