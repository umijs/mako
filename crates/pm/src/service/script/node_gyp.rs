//! node-gyp bootstrap for packages with native addon builds.

use std::env;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;
use tokio::sync::OnceCell;

use super::ScriptService;
use super::exec::status_exit_code;
use crate::error::{CliError, ErrorKind};
use crate::model::cli_output::{CapturedOutput, ErrorDetails, ExecutionStatus, ProcessExecution};
use crate::model::package::PackageInfo;
use crate::util::platform_const::PATH_SEPARATOR;
use crate::util::user_config::get_registry;

/// Cached result of node-gyp availability check and installation
static NODE_GYP_ENSURED: OnceCell<Result<bool, Box<CliError>>> = OnceCell::const_new();

/// Cache structured diagnostics; the CLI presenter owns human/JSON rendering.
async fn install_node_gyp(command: Command) -> Result<bool, Box<CliError>> {
    let cwd = command
        .get_current_dir()
        .map_or_else(env::current_dir, std::path::absolute)
        .map_err(|error| {
            CliError::new(
                ErrorKind::Local,
                format!("Failed to read node-gyp installation directory: {error}"),
            )
            .with_code("process_start_failed")
        })?;
    let mut execution = ProcessExecution {
        command: std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        cwd: cwd.to_string_lossy().into_owned(),
        status: ExecutionStatus::FailedToStart,
        exit_code: None,
        stdout: CapturedOutput::empty(),
        stderr: CapturedOutput::empty(),
        duration_ms: 0,
    };
    let started = Instant::now();
    let output = match ScriptService::run_captured(command, None).await {
        Ok(output) => output,
        Err(error) => {
            execution.duration_ms = started.elapsed().as_millis() as u64;
            return Err(CliError::new(
                ErrorKind::Local,
                format!("Failed to run node-gyp installation: {error:#}"),
            )
            .with_code("process_start_failed")
            .with_details(ErrorDetails::Process { execution })
            .into());
        }
    };

    if output.status.success() {
        tracing::debug!("node-gyp installed successfully");
        return Ok(true);
    }

    execution.status = ExecutionStatus::Failed;
    execution.exit_code = Some(status_exit_code(&output.status) as u32);
    execution.stdout = CapturedOutput::from_bytes(&output.stdout);
    execution.stderr = CapturedOutput::from_bytes(&output.stderr);
    execution.duration_ms = started.elapsed().as_millis() as u64;
    Err(CliError::new(
        ErrorKind::Local,
        format!("Failed to install node-gyp globally: {}", output.status),
    )
    .with_code("process_failed")
    .with_details(ErrorDetails::Process { execution })
    .into())
}

impl ScriptService {
    /// Check if node-gyp exists in PATH by searching directories
    fn has_node_gyp_in_path() -> bool {
        let path_separator = PATH_SEPARATOR;
        env::var("PATH").is_ok_and(|paths| {
            paths
                .split(path_separator)
                .map(|dir| Path::new(dir).join("node-gyp"))
                .any(|path| path.exists())
        })
    }

    /// Ensure node-gyp is available in PATH, install globally if not.
    /// Uses OnceCell to ensure installation happens only once, even with concurrent calls.
    pub async fn ensure_node_gyp() -> Result<bool> {
        let result = NODE_GYP_ENSURED
            .get_or_init(|| async {
                if Self::has_node_gyp_in_path() {
                    tracing::debug!("node-gyp found in PATH");
                    return Ok(true);
                }

                tracing::debug!("node-gyp not found in PATH, installing globally");
                let mut command = Command::new("ut");
                command
                    .args(["i", "-g", "node-gyp", "--registry"])
                    .arg(get_registry());
                install_node_gyp(command).await
            })
            .await;

        result.clone().map_err(|error| anyhow::Error::new(*error))
    }

    pub fn is_node_gyp_pkg(package: &PackageInfo) -> bool {
        // https://hitu.antgroup-inc.cn/packages/@npmcli/node-gyp/files/lib/index.js#L6:L6
        package.path.join("binding.gyp").exists()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::package::LifecycleScripts;

    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_install_node_gyp_preserves_failure_output() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            r#"printf '%s\n' 'resolving node-gyp'
printf '%s\n' \
    'Error: certificate failed' \
    '    at fetch (registry.js:42:7)' \
    'Full logs saved to: /tmp/child.log' >&2
exit 1"#,
        ]);

        let error = install_node_gyp(command).await.unwrap_err();
        assert_eq!(error.code(), "process_failed");
        assert_eq!(
            error.message(),
            "Failed to install node-gyp globally: exit status: 1"
        );
        let details = serde_json::to_value(error.details().unwrap()).unwrap();
        assert_eq!(details["kind"], "process");
        let execution = &details["execution"];
        assert_eq!(execution["status"], "failed");
        assert_eq!(execution["exitCode"], 1);
        assert_eq!(execution["stdout"]["tail"], "resolving node-gyp\n");
        assert_eq!(
            execution["stderr"]["tail"],
            concat!(
                "Error: certificate failed\n",
                "    at fetch (registry.js:42:7)\n",
                "Full logs saved to: /tmp/child.log\n",
            )
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_install_node_gyp_success() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf 'installed\\n'; exit 0"]);

        assert!(install_node_gyp(command).await.unwrap());
    }

    #[tokio::test]
    async fn test_install_node_gyp_spawn_failure() {
        let temp_dir = tempdir().unwrap();
        let command = Command::new(temp_dir.path().join("missing-ut"));

        let error = install_node_gyp(command).await.unwrap_err();
        assert_eq!(error.code(), "process_start_failed");
        assert!(
            error
                .message()
                .contains("Failed to run node-gyp installation:")
        );
        let details = serde_json::to_value(error.details().unwrap()).unwrap();
        let execution = &details["execution"];
        assert_eq!(execution["status"], "failed_to_start");
        assert!(execution["exitCode"].is_null());
        assert_eq!(execution["stdout"]["bytes"], 0);
        assert_eq!(execution["stderr"]["bytes"], 0);
    }

    #[test]
    fn test_has_node_gyp_in_path_found() {
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let node_gyp_path = temp_dir.path().join("node-gyp");
        // Create a dummy node-gyp executable
        fs::write(&node_gyp_path, "#!/bin/sh\necho node-gyp").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&node_gyp_path, fs::Permissions::from_mode(0o755)).unwrap();

        // Save original PATH
        let original_path = env::var("PATH").unwrap_or_default();
        // Set PATH to only include our temp dir
        unsafe {
            env::set_var("PATH", temp_dir.path());
        }

        // Should find node-gyp
        assert!(ScriptService::has_node_gyp_in_path());

        // Restore original PATH
        unsafe {
            env::set_var("PATH", original_path);
        }
    }

    #[tokio::test]
    async fn test_ensure_node_gyp_found() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let node_gyp_path = temp_dir.path().join("node-gyp");
        // Create a dummy node-gyp executable
        fs::write(&node_gyp_path, "#!/bin/sh\necho node-gyp").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&node_gyp_path, fs::Permissions::from_mode(0o755)).unwrap();

        // Save original PATH
        let original_path = env::var("PATH").unwrap_or_default();
        // Set PATH to only include our temp dir
        unsafe {
            env::set_var("PATH", temp_dir.path());
        }
        // Should return Ok(true) because node-gyp exists
        let result = ScriptService::ensure_node_gyp().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
        // Restore original PATH
        unsafe {
            env::set_var("PATH", original_path);
        }
    }

    #[tokio::test]
    async fn test_is_node_gyp_pkg_true_and_false() {
        // Create a temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let package_path = temp_dir.path();
        // Case 1: binding.gyp exists
        let binding_gyp = package_path.join("binding.gyp");
        fs::write(&binding_gyp, "{}").unwrap();
        let package = PackageInfo {
            path: package_path.to_path_buf(),
            bin_files: Default::default(),
            scripts: Default::default(),
            lifecycle_scripts: LifecycleScripts::default(),
            name: "test-package".to_string(),
        };
        assert!(ScriptService::is_node_gyp_pkg(&package));
        // Case 2: binding.gyp does not exist
        fs::remove_file(&binding_gyp).unwrap();
        assert!(!ScriptService::is_node_gyp_pkg(&package));
    }
}
