use crate::compiler::Compiler;
use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use notify::event::DataChange;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::time::Instant;

pub fn start_watch<P: AsRef<Path>>(path: &P, compiler: &mut Compiler) {
    println!("start watching {:?}", path.as_ref().display());

    futures::executor::block_on(async {
        if let Err(e) = async_watch(path, compiler).await {
            println!("error: {:?}", e)
        }
    });
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

async fn async_watch<P: AsRef<Path>>(path: P, c: &mut Compiler) -> notify::Result<()> {
    let (mut watcher, mut rx) = async_watcher()?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => {
                if let EventKind::Modify(notify::event::ModifyKind::Data(DataChange::Any)) =
                    event.kind
                {
                    let project_files = event
                        .paths
                        .iter()
                        .filter(|p| {
                            return !p
                                .strip_prefix(path.as_ref())
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .starts_with("dist")
                                || p.to_str().unwrap().contains("node_modules");
                        })
                        .collect::<Vec<_>>();

                    if !project_files.is_empty() {
                        println!("files changed: {:?} ", project_files);
                        println!("re-compiling...");
                        let start = Instant::now();
                        c.before_rerun();
                        c.run();
                        println!("✅re-compiled {:?}", start.elapsed());
                    }
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}
