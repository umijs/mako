use anyhow::Result;
use crate::service::execute as execute_service;

/// Execute a package binary, similar to npx
pub async fn execute(command: &str, args: Vec<String>) -> Result<()> {
    execute_service::execute_package(command, args).await
}
