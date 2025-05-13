use anyhow::Result;
use std::sync::Arc;
use turbo_tasks::{
    get_effects, Completion, Effects, OperationVc, ReadRef, ResolvedVc, TryJoinIterExt, Vc,
};
use turbopack_core::{
    diagnostics::{Diagnostic, DiagnosticContextExt, PlainDiagnostic},
    issue::{IssueDescriptionExt, PlainIssue},
};

use crate::{entrypoints::Entrypoints, operation::EntrypointsOperation, project::ProjectContainer};

#[turbo_tasks::value(shared, serialization = "none")]
pub struct EntrypointsWithIssues {
    pub entrypoints: ReadRef<EntrypointsOperation>,
    pub issues: Arc<Vec<ReadRef<PlainIssue>>>,
    pub diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    pub effects: Arc<Effects>,
}

#[turbo_tasks::function(operation)]
pub async fn get_entrypoints_with_issues_operation(
    container: ResolvedVc<ProjectContainer>,
) -> Result<Vc<EntrypointsWithIssues>> {
    let entrypoints_operation =
        EntrypointsOperation::new(project_container_entrypoints_operation(container));
    let entrypoints = entrypoints_operation.read_strongly_consistent().await?;
    let issues = get_issues(entrypoints_operation).await?;
    let diagnostics = get_diagnostics(entrypoints_operation).await?;
    let effects = Arc::new(get_effects(entrypoints_operation).await?);
    Ok(EntrypointsWithIssues {
        entrypoints,
        issues,
        diagnostics,
        effects,
    }
    .cell())
}

#[turbo_tasks::function(operation)]
fn project_container_entrypoints_operation(
    // the container is a long-lived object with internally mutable state, there's no risk of it
    // becoming stale
    container: ResolvedVc<ProjectContainer>,
) -> Vc<Entrypoints> {
    container.entrypoints()
}

pub async fn get_issues<T: Send>(source: OperationVc<T>) -> Result<Arc<Vec<ReadRef<PlainIssue>>>> {
    let issues = source.peek_issues_with_path().await?;
    Ok(Arc::new(issues.get_plain_issues().await?))
}

/// Reads the [turbopack_core::diagnostics::Diagnostic] held
/// by the given source and returns it as a
/// [turbopack_core::diagnostics::PlainDiagnostic]. It does
/// not consume any Diagnostics held by the source.
pub async fn get_diagnostics<T: Send>(
    source: OperationVc<T>,
) -> Result<Arc<Vec<ReadRef<PlainDiagnostic>>>> {
    let captured_diags = source.peek_diagnostics().await?;
    let mut diags = captured_diags
        .diagnostics
        .iter()
        .map(|d| d.into_plain())
        .try_join()
        .await?;

    diags.sort();

    Ok(Arc::new(diags))
}

#[turbo_tasks::value(shared, serialization = "none", eq = "manual")]
pub struct EndpointIssuesAndDiags {
    pub changed: Option<ReadRef<Completion>>,
    pub issues: Arc<Vec<ReadRef<PlainIssue>>>,
    pub diagnostics: Arc<Vec<ReadRef<PlainDiagnostic>>>,
    pub effects: Arc<Effects>,
}

impl PartialEq for EndpointIssuesAndDiags {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.changed, &other.changed) {
            (Some(a), Some(b)) => ReadRef::ptr_eq(a, b),
            (None, None) => true,
            (None, Some(_)) | (Some(_), None) => false,
        }) && self.issues == other.issues
            && self.diagnostics == other.diagnostics
    }
}

impl Eq for EndpointIssuesAndDiags {}
