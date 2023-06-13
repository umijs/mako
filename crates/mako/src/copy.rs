use std::path::Path;

use glob::glob;
use tracing::info;

use crate::compiler::Compiler;

impl Compiler {
    // TODO:
    // copy 的文件在 watch 模式下，不应该每次都 copy，而是应该只 copy 发生变化的文件
    pub fn copy(&self) {
        info!("copy");
        let dest = self.context.config.output.path.as_path();
        for src in self.context.config.copy.iter() {
            let src = self.context.root.join(src);
            copy(src.as_path(), dest);
        }
    }
}

fn copy(src: &Path, dest: &Path) {
    let paths = glob(src.to_str().unwrap()).unwrap();

    for entry in paths {
        let entry = entry.unwrap();

        // debug!("copy {:?}", &entry);

        if entry.is_dir() {
            let options = fs_extra::dir::CopyOptions::new()
                .content_only(true)
                .skip_exist(false)
                .overwrite(true);

            fs_extra::dir::copy(&entry, dest, &options).unwrap();
        } else {
            let file_name = entry.file_name().unwrap();
            let options = fs_extra::file::CopyOptions::new()
                .skip_exist(false)
                .overwrite(true);

            fs_extra::file::copy(&entry, dest.join(file_name), &options).unwrap();
        }
    }
}
