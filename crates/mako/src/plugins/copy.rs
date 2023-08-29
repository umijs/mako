use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use glob::glob;
use tracing::debug;

use crate::compiler::Context;
use crate::plugin::Plugin;
use crate::stats::StatsJsonMap;

pub struct CopyPlugin {}

impl Plugin for CopyPlugin {
    fn name(&self) -> &str {
        "copy"
    }

    fn build_success(&self, _stats: &StatsJsonMap, context: &Arc<Context>) -> Result<Option<()>> {
        let dest = context.config.output.path.as_path();
        for src in context.config.copy.iter() {
            let src = context.root.join(src);
            debug!("copy {:?} to {:?}", src, dest);
            copy(src.as_path(), dest)?;
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
