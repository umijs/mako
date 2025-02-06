use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use fs_extra;
use glob::glob;
use notify::event::{CreateKind, DataChange, ModifyKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc::channel;
use tracing::debug;

use crate::ast::file::win_path;
use crate::compiler::Context;
use crate::config::CopyConfig;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;
use crate::utils::tokio_runtime;

pub struct CopyPlugin {}

impl CopyPlugin {
    fn watch(context: &Arc<Context>) {
        let context = context.clone();
        tokio_runtime::spawn(async move {
            let (tx, mut rx) = channel(2);
            let mut watcher = RecommendedWatcher::new(
                move |res| {
                    tx.blocking_send(res).unwrap();
                },
                notify::Config::default(),
            )
            .unwrap();
            for config in context.config.copy.iter() {
                let src = match config {
                    CopyConfig::Basic(src) => context.root.join(src),
                    CopyConfig::Advanced { from, .. } => context.root.join(from),
                };

                if src.exists() {
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
        for config in context.config.copy.iter() {
            match config {
                CopyConfig::Basic(src) => {
                    let src = context.root.join(src);
                    debug!("copy {:?} to {:?}", src, dest);
                    copy(&src, dest)?;
                }

                CopyConfig::Advanced { from, to } => {
                    let src = context.root.join(from);
                    let target = dest.join(to.trim_start_matches("/"));

                    let was_created = if !target.exists() {
                        fs::create_dir_all(&target).is_ok()
                    } else {
                        false
                    };
                    let canonical_target = target.canonicalize()?;
                    let canonical_dest_path = dest.canonicalize()?;
                    if !canonical_target.starts_with(&canonical_dest_path) {
                        if was_created {
                            fs::remove_dir_all(&target)?;
                        }
                        return Err(anyhow!("Invalid target path: {:?}", target));
                    }

                    debug!("copy {:?} to {:?}", src, target);
                    copy(&src, &target)?;
                }
            }
        }
        Ok(())
    }
}

impl Plugin for CopyPlugin {
    fn name(&self) -> &str {
        "copy"
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<()> {
        CopyPlugin::copy(context)?;
        if context.args.watch {
            CopyPlugin::watch(context);
        }
        Ok(())
    }
}

fn copy(src: &Path, dest: &Path) -> Result<()> {
    let src = win_path(src.to_str().unwrap());
    let paths = glob(&src)?;

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
