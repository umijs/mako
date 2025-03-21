use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{trace::TraceRawVcs, NonLocalValue, OperationValue, TaskInput};

use super::WatchOptions;

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    TaskInput,
    PartialEq,
    Eq,
    Hash,
    TraceRawVcs,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: RcStr,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: RcStr,

    /// The contents of bundle.config.js, serialized to JSON.
    pub bundle_config: RcStr,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: RcStr,

    /// A map of environment variables to use when compiling code.
    pub env: Vec<(RcStr, RcStr)>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: DefineEnv,

    /// Filesystem watcher options.
    pub watch: WatchOptions,

    /// The mode in which Next.js is running.
    pub dev: bool,

    /// The build id.
    pub build_id: RcStr,

    /// The browserslist query to use for targeting browsers.
    pub browserslist_query: RcStr,

    /// When the code is minified, this opts out of the default mangling of
    /// local names for variables, functions etc., which can be useful for
    /// debugging/profiling purposes.
    pub no_mangling: bool,
}

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    TaskInput,
    PartialEq,
    Eq,
    Hash,
    TraceRawVcs,
    NonLocalValue,
    OperationValue,
)]
#[serde(rename_all = "camelCase")]
pub struct DefineEnv {
    pub client: Vec<(RcStr, RcStr)>,
    pub edge: Vec<(RcStr, RcStr)>,
    pub nodejs: Vec<(RcStr, RcStr)>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, TaskInput, PartialEq, Eq, Hash, TraceRawVcs, NonLocalValue,
)]
#[serde(rename_all = "camelCase")]
pub struct PartialProjectOptions {
    /// A root path from which all files must be nested under. Trying to access
    /// a file outside this root will fail. Think of this as a chroot.
    pub root_path: Option<RcStr>,

    /// A path inside the root_path which contains the app/pages directories.
    pub project_path: Option<RcStr>,

    /// The contents of next.config.js, serialized to JSON.
    pub bundle_config: Option<RcStr>,

    /// The contents of ts/config read by load-jsconfig, serialized to JSON.
    pub js_config: Option<RcStr>,

    /// A map of environment variables to use when compiling code.
    pub env: Option<Vec<(RcStr, RcStr)>>,

    /// A map of environment variables which should get injected at compile
    /// time.
    pub define_env: Option<DefineEnv>,

    /// Filesystem watcher options.
    pub watch: Option<WatchOptions>,

    /// The mode in which Next.js is running.
    pub dev: Option<bool>,

    /// The build id.
    pub build_id: Option<RcStr>,
}
