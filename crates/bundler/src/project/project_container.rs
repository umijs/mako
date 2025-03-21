use anyhow::{Context, Result};
use turbo_rcstr::RcStr;
use turbo_tasks::{ReadRef, ResolvedVc, State, Vc};
use turbo_tasks_env::EnvMap;
use turbo_tasks_fs::{DiskFileSystem, FileSystemPath};
use turbopack_core::source_map::OptionStringifiedSourceMap;

use crate::{
    config::{bundle_config::Config, js_config::JsConfig, mode::Mode},
    versioned_content_map::VersionedContentMap,
};

use super::{
    entrypoint::Entrypoints,
    project_options::{PartialProjectOptions, ProjectOptions},
    Project, ProjectDefineEnv,
};

#[turbo_tasks::function(operation)]
fn project_fs_operation(project: ResolvedVc<Project>) -> Vc<DiskFileSystem> {
    project.project_fs()
}

#[turbo_tasks::function(operation)]
fn output_fs_operation(project: ResolvedVc<Project>) -> Vc<DiskFileSystem> {
    project.project_fs()
}

#[turbo_tasks::value]
pub struct ProjectContainer {
    name: RcStr,
    options_state: State<Option<ProjectOptions>>,
    versioned_content_map: Option<ResolvedVc<VersionedContentMap>>,
}

#[turbo_tasks::value_impl]
impl ProjectContainer {
    #[turbo_tasks::function]
    pub async fn new(name: RcStr, dev: bool) -> Result<Vc<Self>> {
        Ok(ProjectContainer {
            name,
            // we only need to enable versioning in dev mode, since build
            // is assumed to be operating over a static snapshot
            versioned_content_map: if dev {
                Some(VersionedContentMap::new())
            } else {
                None
            },
            options_state: State::new(None),
        }
        .cell())
    }
}

impl ProjectContainer {
    #[tracing::instrument(level = "info", name = "initialize project", skip_all)]
    pub async fn initialize(self: ResolvedVc<Self>, options: ProjectOptions) -> Result<()> {
        let watch = options.watch;

        self.await?.options_state.set(Some(options));

        let project = self.project().to_resolved().await?;
        let project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        if watch.enable {
            project_fs
                .start_watching_with_invalidation_reason(watch.poll_interval)
                .await?;
        } else {
            project_fs.invalidate_with_reason();
        }
        let output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        output_fs.invalidate_with_reason();
        Ok(())
    }

    #[tracing::instrument(level = "info", name = "update project", skip_all)]
    pub async fn update(self: Vc<Self>, options: PartialProjectOptions) -> Result<()> {
        let PartialProjectOptions {
            root_path,
            project_path,
            bundle_config,
            js_config,
            env,
            define_env,
            watch,
            dev,
            build_id,
        } = options;

        let this = self.await?;

        let mut new_options = this
            .options_state
            .get()
            .clone()
            .context("ProjectContainer need to be initialized with initialize()")?;

        if let Some(root_path) = root_path {
            new_options.root_path = root_path;
        }
        if let Some(project_path) = project_path {
            new_options.project_path = project_path;
        }
        if let Some(bundle_config) = bundle_config {
            new_options.bundle_config = bundle_config;
        }
        if let Some(js_config) = js_config {
            new_options.js_config = js_config;
        }
        if let Some(env) = env {
            new_options.env = env;
        }
        if let Some(define_env) = define_env {
            new_options.define_env = define_env;
        }
        if let Some(watch) = watch {
            new_options.watch = watch;
        }
        if let Some(dev) = dev {
            new_options.dev = dev;
        }

        if let Some(build_id) = build_id {
            new_options.build_id = build_id;
        }

        // TODO: Handle mode switch, should prevent mode being switched.
        let watch = new_options.watch;

        let project = self.project().to_resolved().await?;
        let prev_project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        let prev_output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;

        this.options_state.set(Some(new_options));
        let project = self.project().to_resolved().await?;
        let project_fs = project_fs_operation(project)
            .read_strongly_consistent()
            .await?;
        let output_fs = output_fs_operation(project)
            .read_strongly_consistent()
            .await?;

        if !ReadRef::ptr_eq(&prev_project_fs, &project_fs) {
            if watch.enable {
                // TODO stop watching: prev_project_fs.stop_watching()?;
                project_fs
                    .start_watching_with_invalidation_reason(watch.poll_interval)
                    .await?;
            } else {
                project_fs.invalidate_with_reason();
            }
        }
        if !ReadRef::ptr_eq(&prev_output_fs, &output_fs) {
            prev_output_fs.invalidate_with_reason();
        }

        Ok(())
    }
}

#[turbo_tasks::value_impl]
impl ProjectContainer {
    #[turbo_tasks::function]
    pub async fn project(&self) -> Result<Vc<Project>> {
        let env_map: Vc<EnvMap>;
        let bundle_config;
        let define_env;
        let js_config;
        let root_path;
        let project_path;
        let watch;
        let dev;

        let browserslist_query;
        let no_mangling;
        {
            let options = self.options_state.get();
            let options = options
                .as_ref()
                .context("ProjectContainer need to be initialized with initialize()")?;
            env_map = Vc::cell(options.env.iter().cloned().collect());
            define_env = ProjectDefineEnv {
                client: ResolvedVc::cell(options.define_env.client.iter().cloned().collect()),
                edge: ResolvedVc::cell(options.define_env.edge.iter().cloned().collect()),
                nodejs: ResolvedVc::cell(options.define_env.nodejs.iter().cloned().collect()),
            }
            .cell();
            bundle_config = Config::from_string(Vc::cell(options.bundle_config.clone()));
            js_config = JsConfig::from_string(Vc::cell(options.js_config.clone()));
            root_path = options.root_path.clone();
            project_path = options.project_path.clone();
            watch = options.watch;
            dev = options.dev;
            browserslist_query = options.browserslist_query.clone();
            no_mangling = options.no_mangling
        }

        let dist_dir = bundle_config
            .await?
            .dist_dir
            .as_ref()
            .map_or_else(|| "dist".into(), |d| d.clone());

        Ok(Project {
            root_path,
            project_path,
            watch,
            config: bundle_config.to_resolved().await?,
            js_config: js_config.to_resolved().await?,
            dist_dir,
            env: ResolvedVc::upcast(env_map.to_resolved().await?),
            define_env: define_env.to_resolved().await?,
            browserslist_query,
            mode: if dev {
                Mode::Development.resolved_cell()
            } else {
                Mode::Build.resolved_cell()
            },
            versioned_content_map: self.versioned_content_map,
            no_mangling,
        }
        .cell())
    }

    /// See [Project::entrypoints].
    #[turbo_tasks::function]
    pub fn entrypoints(self: Vc<Self>) -> Vc<Entrypoints> {
        self.project().entrypoints()
    }

    /// See [Project::hmr_identifiers].
    #[turbo_tasks::function]
    pub fn hmr_identifiers(self: Vc<Self>) -> Vc<Vec<RcStr>> {
        self.project().hmr_identifiers()
    }

    /// Gets a source map for a particular `file_path`. If `dev` mode is disabled, this will always
    /// return [`OptionStringifiedSourceMap::none`].
    #[turbo_tasks::function]
    pub fn get_source_map(
        &self,
        file_path: Vc<FileSystemPath>,
        section: Option<RcStr>,
    ) -> Vc<OptionStringifiedSourceMap> {
        if let Some(map) = self.versioned_content_map {
            map.get_source_map(file_path, section)
        } else {
            OptionStringifiedSourceMap::none()
        }
    }
}
