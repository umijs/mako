use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use deno_core;
use deno_resolver;
use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{BootstrapOptions, WorkerExecutionMode};
use deno_runtime::colors::ColorLevel;
use deno_runtime::deno_core::{resolve_import, FsModuleLoader, ModuleSpecifier};
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_node::NodeExtInitServices;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use sys_traits;

deno_core::extension!(
  hello_runtime,
  // ops = [],
  // esm_entry_point = "ext:hello_runtime/bootstrap.js",
  // esm = [dir "examples/extension", "bootstrap.js"]
);

pub async fn boostrap(main_js_path: &str) -> Result<(), AnyError> {
    let js_path = Path::new(main_js_path);
    let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();
    let fs = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(
        RuntimePermissionDescriptorParser::new(sys_traits::impls::RealSys),
    );

    let options = BootstrapOptions {
        deno_version: "1.0.0".to_string(),
        args: vec![],
        cpu_count: 0,
        log_level: Default::default(),
        enable_op_summary_metrics: false,
        enable_testing_features: false,
        locale: "".to_string(),
        location: None,
        no_color: false,
        is_stdout_tty: false,
        is_stderr_tty: false,
        color_level: ColorLevel::None,
        unstable_features: vec![],
        user_agent: "".to_string(),
        inspect: false,
        has_node_modules_dir: false,
        argv0: None,
        node_debug: None,
        node_ipc_fd: None,
        mode: WorkerExecutionMode::None,
        serve_port: None,
        serve_host: None,
        otel_config: Default::default(),
        close_on_idle: false,
    };
    // let worker = MainWorker
    let mut worker = MainWorker::bootstrap_from_options(
        &main_module,
        WorkerServiceOptions::<
            DenoInNpmPackageChecker,
            NpmResolver<sys_traits::impls::RealSys>,
            sys_traits::impls::RealSys,
        > {
            module_loader: Rc::new(FsModuleLoader),
            permissions: PermissionsContainer::allow_all(permission_desc_parser),
            blob_store: Default::default(),
            broadcast_channel: Default::default(),
            feature_checker: Default::default(),
            node_services: Default::default(),
            // node_services: Some(NodeExtInitServices {
            //     node_require_loader,
            //     node_resolver: self.node_resolver.clone(),
            //     pkg_json_resolver: self.pkg_json_resolver.clone(),
            //     sys: self.sys.clone(),
            // }),
            npm_process_state_provider: Default::default(),
            root_cert_store_provider: Default::default(),
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: Default::default(),
            compiled_wasm_module_store: Default::default(),
            v8_code_cache: Default::default(),
            fs,
        },
        WorkerOptions {
            // extensions: vec![hello_runtime::init_ops_and_esm()],
            ..Default::default()
        },
    );
    worker.execute_main_module(&main_module).await?;
    worker.run_event_loop(false).await?;
    Ok(())
    // let worker = MainWorker::bootstrap_from_options(&url, options);
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {
        println!("hello");
    }
}