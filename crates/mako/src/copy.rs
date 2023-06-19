use std::path::Path;

use anyhow::Result;
use glob::glob;

use crate::compiler::Compiler;

impl Compiler {
    // TODO:
    // copy 的文件在 watch 模式下，不应该每次都 copy，而是应该只 copy 发生变化的文件
    pub fn copy(&self) -> Result<()> {
        let dest = self.context.config.output.path.as_path();
        for src in self.context.config.copy.iter() {
            let src = self.context.root.join(src);
            copy(src.as_path(), dest)?;
        }
        Ok(())
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
