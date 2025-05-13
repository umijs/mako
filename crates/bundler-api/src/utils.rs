use std::sync::Arc;

use anyhow::Result;
use turbo_tasks::{get_effects, Completion, Effects, OperationVc, ReadRef, Vc, VcValueType};
use turbopack_core::{
    diagnostics::PlainDiagnostic,
    issue::{IssueSeverity, PlainIssue},
};

use crate::{
    endpoints::{endpoint_server_changed_operation, Endpoint},
    issues::{get_diagnostics, get_issues, EndpointIssuesAndDiags},
};

#[turbo_tasks::function(operation)]
pub async fn subscribe_issues_and_diags_operation(
    endpoint_op: OperationVc<Box<dyn Endpoint>>,
    should_include_issues: bool,
) -> Result<Vc<EndpointIssuesAndDiags>> {
    let changed_op = endpoint_server_changed_operation(endpoint_op);

    if should_include_issues {
        let (changed_value, issues, diagnostics, effects) =
            strongly_consistent_catch_collectables(changed_op).await?;
        Ok(EndpointIssuesAndDiags {
            changed: changed_value,
            issues,
            diagnostics,
            effects,
        }
        .cell())
    } else {
        let changed_value = changed_op.read_strongly_consistent().await?;
        Ok(EndpointIssuesAndDiags {
            changed: Some(changed_value),
            issues: Arc::new(vec![]),
            diagnostics: Arc::new(vec![]),
            effects: Arc::new(Effects::default()),
        }
        .cell())
    }
}

#[turbo_tasks::function(operation)]
pub fn endpoint_client_changed_operation(
    endpoint_op: OperationVc<Box<dyn Endpoint>>,
) -> Vc<Completion> {
    endpoint_op.connect().client_changed()
}

// Await the source and return fatal issues if there are any, otherwise
// propagate any actual error results.
pub async fn strongly_consistent_catch_collectables<R: VcValueType + Send>(
    source_op: OperationVc<R>,
) -> Result<(
    Option<ReadRef<R>>,
    Arc<Vec<ReadRef<PlainIssue>>>,
    Arc<Vec<ReadRef<PlainDiagnostic>>>,
    Arc<Effects>,
)> {
    let result = source_op.read_strongly_consistent().await;
    let issues = get_issues(source_op).await?;
    let diagnostics = get_diagnostics(source_op).await?;
    let effects = Arc::new(get_effects(source_op).await?);

    let result = if result.is_err() && issues.iter().any(|i| i.severity <= IssueSeverity::Error) {
        None
    } else {
        Some(result?)
    };

    Ok((result, issues, diagnostics, effects))
}
