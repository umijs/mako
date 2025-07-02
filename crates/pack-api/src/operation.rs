use anyhow::Result;
use serde::{Deserialize, Serialize};
use turbo_tasks::{
    CollectiblesSource, NonLocalValue, OperationVc, ResolvedVc, Vc, debug::ValueDebugFormat,
    get_effects, trace::TraceRawVcs,
};
use turbopack_core::{diagnostics::Diagnostic, issue::IssueDescriptionExt};

use crate::{endpoint::Endpoint, entrypoint::Entrypoints};

/// Based on [`Entrypoints`], but with [`OperationVc<Endpoint>`][OperationVc] for every endpoint.
///
/// This is used when constructing `ExternalEndpoint`s in the `napi` crate.
///
/// This is important as `OperationVc`s can be stored in the VersionedContentMap and can be exposed
/// to JS via napi.
///
/// This is needed to call `write_to_disk` which expects an `OperationVc<Endpoint>`.
#[turbo_tasks::value(shared)]
pub struct EntrypointsOperation {
    pub apps: Option<AppOperation>,
    pub libraries: Option<LibraryOperation>,
}

/// HACK: Wraps an `OperationVc<Entrypoints>` inside of a second `OperationVc`.
#[turbo_tasks::function(operation)]
fn entrypoints_wrapper(entrypoints: OperationVc<Entrypoints>) -> Vc<Entrypoints> {
    entrypoints.connect()
}

/// Removes diagnostics, issues, and effects from the top-level `entrypoints` operation so that
/// they're not duplicated across many different individual entrypoints or routes.
#[turbo_tasks::function(operation)]
async fn entrypoints_without_collectibles_operation(
    entrypoints: OperationVc<Entrypoints>,
) -> Result<Vc<Entrypoints>> {
    let _ = entrypoints.resolve_strongly_consistent().await?;
    let _ = entrypoints.take_collectibles::<Box<dyn Diagnostic>>();
    let _ = entrypoints.take_issues_with_path().await?;
    let _ = get_effects(entrypoints).await?;
    Ok(entrypoints.connect())
}

#[turbo_tasks::value_impl]
impl EntrypointsOperation {
    #[turbo_tasks::function(operation)]
    pub async fn new(entrypoints: OperationVc<Entrypoints>) -> Result<Vc<Self>> {
        let e = entrypoints.connect().await?;
        let entrypoints = entrypoints_without_collectibles_operation(entrypoints);
        Ok(Self {
            apps: match e.apps.as_ref() {
                Some(es) => {
                    let endpoints: Vec<_> =
                        es.await?.iter().map(|e| wrap(*e, entrypoints)).collect();

                    Some(AppOperation(endpoints))
                }
                None => None,
            },
            libraries: match e.libraries.as_ref() {
                Some(es) => {
                    let endpoints: Vec<_> =
                        es.await?.iter().map(|e| wrap(*e, entrypoints)).collect();

                    Some(LibraryOperation(endpoints))
                }
                None => None,
            },
        }
        .cell())
    }
}

/// Given a resolved `Endpoint` and the `Entrypoints` operation that it comes from, connect the
/// operation and return a `OperationVc` of the `Entrypoint`. This `Endpoint` operation will keep
/// the entire `Entrypoints` operation alive.
#[turbo_tasks::function(operation)]
fn wrap(
    endpoint: ResolvedVc<Box<dyn Endpoint>>,
    op: OperationVc<Entrypoints>,
) -> Vc<Box<dyn Endpoint>> {
    let _ = op.connect();
    *endpoint
}

#[derive(
    TraceRawVcs,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    ValueDebugFormat,
    Clone,
    Debug,
    NonLocalValue,
)]
pub struct LibraryOperation(pub Vec<OperationVc<Box<dyn Endpoint>>>);

#[derive(
    TraceRawVcs,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    ValueDebugFormat,
    Clone,
    Debug,
    NonLocalValue,
)]
pub struct AppOperation(pub Vec<OperationVc<Box<dyn Endpoint>>>);
