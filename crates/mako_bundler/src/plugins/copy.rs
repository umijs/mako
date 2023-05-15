use fs_extra::{
    dir::{copy as copy_dir, CopyOptions as CopyDirOptions},
    file::{copy as copy_file, CopyOptions as CopyFileOptions},
};
use glob::glob;
use std::fs::create_dir;
use std::{path::Path, sync::Arc};
use tracing::debug;

use crate::{
    context::Context,
    generate::generate::GenerateParam,
    plugin::{Plugin, Result},
};

pub struct CopyPlugin;

impl Plugin for CopyPlugin {
    fn name(&self) -> &str {
        "mako:plugin-copy"
    }

    fn generate_end(
        &self,
        context: &Arc<Context>,
        generate_param: &GenerateParam,
    ) -> Result<Option<()>> {
        if !generate_param.write {
            return Ok(None);
        }

        let dest = &context.config.output.path.as_path();
        if !dest.exists() {
            create_dir(dest).unwrap();
        }

        for src in context.config.copy.iter() {
            let src = &context.config.root.join(src);
            copy(src.as_path(), dest);
        }

        Ok(None)
    }
}

fn copy(src: &Path, dest: &Path) {
    let paths = glob(src.to_str().unwrap()).unwrap();

    for entry in paths {
        let entry = entry.unwrap();

        debug!("copy {:?}", &entry);

        if entry.is_dir() {
            let options = CopyDirOptions::new()
                .content_only(true)
                .skip_exist(false)
                .overwrite(true);

            copy_dir(&entry, dest, &options).unwrap();
        } else {
            let file_name = entry.file_name().unwrap();
            let options = CopyFileOptions::new().skip_exist(false).overwrite(true);

            copy_file(&entry, dest.join(file_name), &options).unwrap();
        }
    }
}
