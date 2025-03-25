use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::resolve::options::ImportMapping;

pub fn mdx_import_source_file() -> RcStr {
    unreachable!()
}

#[turbo_tasks::function]
pub async fn get_postcss_package_mapping(
    project_path: ResolvedVc<FileSystemPath>,
) -> Result<Vc<ImportMapping>> {
    Ok(
        ImportMapping::Alternatives(vec![ImportMapping::PrimaryAlternative(
            "postcss".into(),
            Some(project_path),
        )
        .resolved_cell()])
        .cell(),
    )
}
