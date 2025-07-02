use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::Vc;
use turbo_tasks_fs::{embed_directory, FileContent, FileSystem, FileSystemPath};

#[turbo_tasks::function]
fn embed_fs() -> Vc<Box<dyn FileSystem>> {
    embed_directory!("@utoo/pack-runtime", "$CARGO_MANIFEST_DIR/js/src")
}

#[turbo_tasks::function]
pub(crate) async fn embed_file(path: RcStr) -> Result<Vc<FileContent>> {
    Ok(embed_fs().root().await?.join(&path)?.read())
}

#[turbo_tasks::function]
pub(crate) async fn embed_file_path(path: RcStr) -> Result<Vc<FileSystemPath>> {
    Ok(embed_fs().root().await?.join(&path)?.cell())
}
