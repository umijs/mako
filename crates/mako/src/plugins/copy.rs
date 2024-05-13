use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::fs_extra;
use mako_core::glob::glob;
#[cfg(not(target_arch = "wasm32"))]
use mako_core::notify::event::{CreateKind, DataChange, ModifyKind, RenameMode};
#[cfg(not(target_arch = "wasm32"))]
use mako_core::notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use mako_core::tokio::sync::mpsc::channel;
use mako_core::tracing::debug;

use crate::compiler::Context;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;
use crate::utils::tokio_runtime;

#[cfg(not(target_arch = "wasm32"))]
pub struct CopyPlugin {}

#[cfg(not(target_arch = "wasm32"))]
impl CopyPlugin {
    fn watch(context: &Arc<Context>) {
        let context = context.clone();
        tokio_runtime::spawn(async move {
            let (tx, mut rx) = channel(2);
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    tx.blocking_send(res).unwrap();
                },
                mako_core::notify::Config::default(),
            )
            .unwrap();
            for src in context.config.copy.iter() {
                let src = context.root.join(src);
                #[cfg(not(target_arch = "wasm32"))]
                let exists = src.exists();
                #[cfg(target_arch = "wasm32")]
                let exists = file::file_exists(src.to_str().unwrap());

                if exists {
                    debug!("watch {:?}", src);
                    let mode = if src.is_dir() {
                        RecursiveMode::Recursive
                    } else {
                        RecursiveMode::NonRecursive
                    };
                    watcher.watch(src.as_path(), mode).unwrap();
                }
            }
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(event) => {
                        if let EventKind::Create(CreateKind::File)
                        | EventKind::Modify(ModifyKind::Data(DataChange::Any))
                        | EventKind::Modify(ModifyKind::Name(RenameMode::Any)) = event.kind
                        {
                            CopyPlugin::copy(&context).unwrap();
                        }
                    }
                    Err(e) => {
                        eprintln!("watch error: {:?}", e);
                    }
                }
            }
        });
    }

    fn copy(context: &Arc<Context>) -> Result<()> {
        debug!("copy");
        let dest = context.config.output.path.as_path();
        for src in context.config.copy.iter() {
            let src = context.root.join(src);
            debug!("copy {:?} to {:?}", src, dest);
            copy(src.as_path(), dest)?;
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Plugin for CopyPlugin {
    fn name(&self) -> &str {
        "copy"
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        CopyPlugin::copy(context)?;
        if context.args.watch {
            CopyPlugin::watch(context);
        }
        Ok(None)
    }
}


fn copy(src: &Path, dest: &Path) -> Result<()> {
    let paths = glob(src.to_str().unwrap())?;

    for entry in paths {
        let entry = entry.unwrap();

        if entry.is_dir() {
            let options = fs_extra::dir::CopyOptions::new()
                .content_only(true)
                .skip_exist(false)
                .overwrite(true);
            fs_extra::dir::copy(&entry, dest, &options)?;
        } else {
            let file_name = entry.file_name().unwrap();
            let options = fs_extra::file::CopyOptions::new()
                .skip_exist(false)
                .overwrite(true);
            fs_extra::file::copy(&entry, dest.join(file_name), &options)?;
        }
    }
    Ok(())
}
