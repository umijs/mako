use turbo_tasks::ResolvedVc;

use crate::endpoints::Endpoints;

#[turbo_tasks::value(shared)]
pub struct Entrypoints {
    pub libraries: Option<ResolvedVc<Endpoints>>,
}
