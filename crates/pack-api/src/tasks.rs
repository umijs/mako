use std::{future::Future, sync::Arc, time::Duration};

use anyhow::Result;
use turbo_tasks::{
    task_statistics::TaskStatisticsApi, trace::TraceRawVcs, TaskId, TurboTasks, TurboTasksApi,
    UpdateInfo, Vc,
};
use turbo_tasks_backend::{DefaultBackingStorage, NoopBackingStorage, TurboTasksBackend};

#[derive(Clone)]
pub enum BundlerTurboTasks {
    Memory(Arc<TurboTasks<TurboTasksBackend<NoopBackingStorage>>>),
    PersistentCaching(Arc<TurboTasks<TurboTasksBackend<DefaultBackingStorage>>>),
}

impl BundlerTurboTasks {
    pub fn dispose_root_task(&self, task: TaskId) {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.dispose_root_task(task),
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => {
                turbo_tasks.dispose_root_task(task)
            }
        }
    }

    pub fn spawn_root_task<T, F, Fut>(&self, functor: F) -> TaskId
    where
        T: Send,
        F: Fn() -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<Vc<T>>> + Send,
    {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.spawn_root_task(functor),
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => {
                turbo_tasks.spawn_root_task(functor)
            }
        }
    }

    pub async fn run_once<T: TraceRawVcs + Send + 'static>(
        &self,
        future: impl Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.run_once(future).await,
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => turbo_tasks.run_once(future).await,
        }
    }

    pub fn spawn_once_task<T, Fut>(&self, future: Fut) -> TaskId
    where
        T: Send,
        Fut: Future<Output = Result<Vc<T>>> + Send + 'static,
    {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.spawn_once_task(future),
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => {
                turbo_tasks.spawn_once_task(future)
            }
        }
    }

    pub async fn aggregated_update_info(
        &self,
        aggregation: Duration,
        timeout: Duration,
    ) -> Option<UpdateInfo> {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => {
                turbo_tasks
                    .aggregated_update_info(aggregation, timeout)
                    .await
            }
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => {
                turbo_tasks
                    .aggregated_update_info(aggregation, timeout)
                    .await
            }
        }
    }

    pub async fn get_or_wait_aggregated_update_info(&self, aggregation: Duration) -> UpdateInfo {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => {
                turbo_tasks
                    .get_or_wait_aggregated_update_info(aggregation)
                    .await
            }
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => {
                turbo_tasks
                    .get_or_wait_aggregated_update_info(aggregation)
                    .await
            }
        }
    }

    pub async fn stop_and_wait(&self) {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.stop_and_wait().await,
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => turbo_tasks.stop_and_wait().await,
        }
    }

    pub fn task_statistics(&self) -> &TaskStatisticsApi {
        match self {
            BundlerTurboTasks::Memory(turbo_tasks) => turbo_tasks.task_statistics(),
            BundlerTurboTasks::PersistentCaching(turbo_tasks) => turbo_tasks.task_statistics(),
        }
    }
}
