#![feature(future_join)]
#![feature(min_specialization)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

use std::time::Instant;

use anyhow::Result;
use bundler_api::{
    endpoints::{endpoint_write_to_disk, Endpoint, EndpointOutputPaths},
    project::{ProjectContainer, ProjectOptions},
};
use clap::Parser;
use futures_util::{StreamExt, TryStreamExt};
use turbo_tasks::{get_effects, ReadConsistency, ResolvedVc, TransientInstance, TurboTasks, Vc};
use turbo_tasks_backend::{NoopBackingStorage, TurboTasksBackend};
use turbo_tasks_malloc::TurboMalloc;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
pub enum Command {
    Build,
    Dev,
}

pub async fn main_inner(
    tt: &TurboTasks<TurboTasksBackend<NoopBackingStorage>>,
    options: ProjectOptions,
) -> Result<()> {
    register();

    let dev = options.dev;

    let project = tt
        .run_once(async {
            let project = ProjectContainer::new("bundler-cli-test".into(), options.dev);
            let project = project.to_resolved().await?;
            project.initialize(options).await?;
            Ok(project)
        })
        .await?;

    tracing::info!("collecting endpoints");
    let entrypoints = tt
        .run_once(async move {
            let mut endpoints: Vec<ResolvedVc<Box<dyn Endpoint>>> = vec![];
            let entrypoints = project.entrypoints().await?;
            if let Some(libraries) = entrypoints.libraries {
                endpoints.extend(libraries.await?.into_iter());
            }
            Ok(endpoints)
        })
        .await?;

    let start = Instant::now();
    let count = render_endpoints(tt, entrypoints).await?;
    tracing::info!("rendered {} entries in {:?}", count, start.elapsed());

    if dev {
        hmr(tt, *project).await?;
    }

    Ok(())
}

pub fn register() {
    bundler_api::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}

pub async fn render_endpoints(
    tt: &TurboTasks<TurboTasksBackend<NoopBackingStorage>>,
    endpoints: Vec<ResolvedVc<Box<dyn Endpoint>>>,
) -> Result<usize> {
    let count = endpoints.len();
    tracing::info!("rendering {} entries", count);

    tokio_stream::iter(endpoints)
        .map(move |library| async move {
            let start = Instant::now();

            tt.run_once({
                async move {
                    endpoint_write_to_disk_with_effects(*library).await?;
                    Ok(())
                }
            })
            .await?;

            let duration = start.elapsed();
            let memory_after = TurboMalloc::memory_usage();

            tracing::info!("{:?} {} MiB", duration, memory_after / 1024 / 1024);

            Ok::<_, anyhow::Error>(())
        })
        .buffer_unordered(count)
        .try_collect::<Vec<_>>()
        .await?;

    Ok(count)
}

#[turbo_tasks::function]
async fn endpoint_write_to_disk_with_effects(
    endpoint: ResolvedVc<Box<dyn Endpoint>>,
) -> Result<Vc<EndpointOutputPaths>> {
    let op = endpoint_write_to_disk_operation(endpoint);
    let result = op.resolve_strongly_consistent().await?;
    get_effects(op).await?.apply().await?;
    Ok(*result)
}

#[turbo_tasks::function(operation)]
pub fn endpoint_write_to_disk_operation(
    endpoint: ResolvedVc<Box<dyn Endpoint>>,
) -> Vc<EndpointOutputPaths> {
    endpoint_write_to_disk(*endpoint)
}

async fn hmr(
    tt: &TurboTasks<TurboTasksBackend<NoopBackingStorage>>,
    project: Vc<ProjectContainer>,
) -> Result<()> {
    tracing::info!("HMR...");
    let session = TransientInstance::new(());
    let idents = tt
        .run_once(async move { project.hmr_identifiers().await })
        .await?;
    let start = Instant::now();
    for ident in idents {
        if !ident.ends_with(".js") {
            continue;
        }
        let session = session.clone();
        let start = Instant::now();
        let task = tt.spawn_root_task(move || {
            let session = session.clone();
            async move {
                let project = project.project();
                let state = project.hmr_version_state(ident.clone(), session);
                project.hmr_update(ident.clone(), state).await?;
                Ok(Vc::<()>::cell(()))
            }
        });
        tt.wait_task_completion(task, ReadConsistency::Strong)
            .await?;
        let e = start.elapsed();
        if e.as_millis() > 10 {
            tracing::info!("HMR: {:?} {:?}", ident, e);
        }
    }
    tracing::info!("HMR {:?}", start.elapsed());

    Ok(())
}
