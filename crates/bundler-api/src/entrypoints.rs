use turbo_tasks::ResolvedVc;

use crate::endpoints::Endpoint;

#[turbo_tasks::value(shared)]
pub struct Entrypoints {
    pub libraries: Vec<ResolvedVc<Box<dyn Endpoint>>>,
}
