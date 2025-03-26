use turbo_rcstr::RcStr;
use turbo_tasks::ResolvedVc;
use turbo_tasks_fs::FileSystemPath;

use crate::project::Project;

#[turbo_tasks::value]
pub struct Library {
    entry: ResolvedVc<FileSystemPath>,
    filename: Option<RcStr>,
    root: RcStr,
    export: Vec<RcStr>,
}

#[turbo_tasks::value]
pub struct LibraryProject {
    project: ResolvedVc<Project>,
    libraries: ResolvedVc<Vec<Library>>,
}

#[turbo_tasks::value(transparent)]
pub struct OptionLibraryProject(Option<ResolvedVc<LibraryProject>>);

impl LibraryProject {}
