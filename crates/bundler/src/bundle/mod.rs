use std::{net::IpAddr, sync::Arc};

use turbo_rcstr::RcStr;
use turbo_tasks::{TurboTasks, Vc};
use turbo_tasks_backend::{NoopBackingStorage, TurboTasksBackend};
use turbopack_core::{
    chunk::{MinifyType, SourceMapsType},
    issue::{IssueReporter, IssueSeverity},
};

use crate::{arguments::Target, util::EntryRequest};

pub mod build;
pub mod dev;

pub(crate) type Backend = TurboTasksBackend<NoopBackingStorage>;

pub struct UtooBundlerBuilder {
    turbo_tasks: Arc<TurboTasks<Backend>>,
    project_dir: RcStr,
    root_dir: RcStr,
    entry_requests: Vec<EntryRequest>,
    browserslist_query: RcStr,
    log_level: IssueSeverity,
    show_all: bool,
    log_detail: bool,

    // For build only
    minify_type: MinifyType,
    // For build only
    target: Target,
    // For build only
    source_maps_type: SourceMapsType,

    // For dev only
    eager_compile: bool,
    // For dev only
    hostname: Option<IpAddr>,
    // For dev only
    port: Option<u16>,
    // For dev only
    allow_retry: bool,
    // For dev only
    issue_reporter: Option<Box<dyn IssueReporterProvider>>,
}

impl UtooBundlerBuilder {
    pub fn new(
        turbo_tasks: Arc<TurboTasks<Backend>>,
        project_dir: RcStr,
        root_dir: RcStr,
    ) -> UtooBundlerBuilder {
        UtooBundlerBuilder {
            turbo_tasks,
            project_dir,
            root_dir,
            entry_requests: vec![],
            eager_compile: false,
            hostname: None,
            issue_reporter: None,
            port: None,
            browserslist_query: "last 1 Chrome versions, last 1 Firefox versions, last 1 Safari \
                                 versions, last 1 Edge versions"
                .into(),
            log_level: IssueSeverity::Warning,
            show_all: false,
            log_detail: false,
            allow_retry: false,
            source_maps_type: SourceMapsType::Full,
            minify_type: MinifyType::Minify { mangle: true },
            target: Target::Node,
        }
    }

    pub fn entry_request(mut self, entry_asset_path: EntryRequest) -> UtooBundlerBuilder {
        self.entry_requests.push(entry_asset_path);
        self
    }

    pub fn eager_compile(mut self, eager_compile: bool) -> UtooBundlerBuilder {
        self.eager_compile = eager_compile;
        self
    }

    pub fn hostname(mut self, hostname: IpAddr) -> UtooBundlerBuilder {
        self.hostname = Some(hostname);
        self
    }

    pub fn port(mut self, port: u16) -> UtooBundlerBuilder {
        self.port = Some(port);
        self
    }

    pub fn browserslist_query(mut self, browserslist_query: RcStr) -> UtooBundlerBuilder {
        self.browserslist_query = browserslist_query;
        self
    }

    pub fn log_level(mut self, log_level: IssueSeverity) -> UtooBundlerBuilder {
        self.log_level = log_level;
        self
    }

    pub fn show_all(mut self, show_all: bool) -> UtooBundlerBuilder {
        self.show_all = show_all;
        self
    }

    pub fn allow_retry(mut self, allow_retry: bool) -> UtooBundlerBuilder {
        self.allow_retry = allow_retry;
        self
    }

    pub fn log_detail(mut self, log_detail: bool) -> UtooBundlerBuilder {
        self.log_detail = log_detail;
        self
    }

    pub fn issue_reporter(
        mut self,
        issue_reporter: Box<dyn IssueReporterProvider>,
    ) -> UtooBundlerBuilder {
        self.issue_reporter = Some(issue_reporter);
        self
    }
}

pub trait IssueReporterProvider: Send + Sync + 'static {
    fn get_issue_reporter(&self) -> Vc<Box<dyn IssueReporter>>;
}

impl<T> IssueReporterProvider for T
where
    T: Fn() -> Vc<Box<dyn IssueReporter>> + Send + Sync + Clone + 'static,
{
    fn get_issue_reporter(&self) -> Vc<Box<dyn IssueReporter>> {
        self()
    }
}
