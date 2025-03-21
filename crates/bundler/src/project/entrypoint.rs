use turbo_rcstr::RcStr;
use turbo_tasks::FxIndexMap;

use crate::endpoint::Route;

#[turbo_tasks::value(shared)]
pub struct Entrypoints {
    pub library: FxIndexMap<RcStr, RcStr>,
    pub routes: FxIndexMap<RcStr, Route>,
}
