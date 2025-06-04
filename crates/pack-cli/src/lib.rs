#![feature(future_join)]
#![feature(min_specialization)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

use std::time::Instant;

use anyhow::Result;
use pack_api::{
    entrypoints::get_all_written_entrypoints_with_issues_operation,
    issues::EntrypointsWithIssues,
    project::{ProjectContainer, ProjectOptions},
};
use clap::{Parser, ValueEnum};
use turbo_tasks::{ReadConsistency, TransientInstance, TurboTasks, Vc};
use turbo_tasks_backend::{NoopBackingStorage, TurboTasksBackend};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Command {
    #[arg(short, long)]
    pub mode: Mode,

    #[arg(short, long)]
    pub project_dir: String,

    #[arg(short, long)]
    pub root_dir: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Mode {
    Build,
    Dev,
}

pub async fn main_inner(
    turbo_tasks: &TurboTasks<TurboTasksBackend<NoopBackingStorage>>,
    options: ProjectOptions,
) -> Result<()> {
    register();

    let dev = options.dev;

    tracing::info!(
        "bundling with {} mode",
        if dev { "development" } else { "production" }
    );

    let start = Instant::now();

    let project_container = turbo_tasks
        .run_once(async move {
            let project_container = ProjectContainer::new("utoo-pack-cli".into(), dev);
            let project_container = project_container.to_resolved().await?;
            project_container.initialize(options).await?;
            Ok(project_container)
        })
        .await?;

    let (entrypoints, _issues, _diagnostics) = turbo_tasks
        .run_once(async move {
            let entrypoints_with_issues_op =
                get_all_written_entrypoints_with_issues_operation(project_container);

            let EntrypointsWithIssues {
                entrypoints,
                issues,
                diagnostics,
                effects,
            } = &*entrypoints_with_issues_op
                .read_strongly_consistent()
                .await?;
            effects.apply().await?;

            Ok((entrypoints.clone(), issues.clone(), diagnostics.clone()))
        })
        .await?;

    tracing::info!("all project entrypoints wrote to disk.");

    tracing::info!(
        "pack tasks with {} apps {} libraries finished in {:?}",
        entrypoints
            .apps
            .as_ref()
            .map(|apps| apps.0.len())
            .unwrap_or_default(),
        entrypoints
            .libraries
            .as_ref()
            .map(|libraries| libraries.0.len())
            .unwrap_or_default(),
        start.elapsed()
    );

    if dev {
        hmr_once(turbo_tasks, *project_container).await?;
    }

    Ok(())
}

async fn hmr_once(
    turbo_tasks: &TurboTasks<TurboTasksBackend<NoopBackingStorage>>,
    project_container: Vc<ProjectContainer>,
) -> Result<()> {
    tracing::info!("HMR...");
    let session = TransientInstance::new(());
    let idents = turbo_tasks
        .run_once(async move { project_container.hmr_identifiers().await })
        .await?;
    let start = Instant::now();
    for ident in idents {
        if !ident.ends_with(".js") {
            continue;
        }
        let session = session.clone();
        let start = Instant::now();
        let task = turbo_tasks.spawn_root_task(move || {
            let session = session.clone();
            async move {
                let project = project_container.project();
                let state = project.hmr_version_state(ident.clone(), session);
                project.hmr_update(ident.clone(), state).await?;
                Ok(Vc::<()>::cell(()))
            }
        });
        turbo_tasks
            .wait_task_completion(task, ReadConsistency::Strong)
            .await?;
        tracing::info!("HMR: {:?} {:?}", ident, start.elapsed());
    }
    tracing::info!("HMR {:?}", start.elapsed());

    Ok(())
}

pub fn register() {
    pack_api::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}
