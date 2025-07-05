mod module_loader;

use deno_core;
use deno_resolver;
use deno_resolver::npm::{ByonmNpmResolverCreateOptions, CreateInNpmPkgCheckerOptions, DenoInNpmPackageChecker, NpmResolver, NpmResolverCreateOptions};
use deno_runtime::colors::ColorLevel;
use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::{resolve_import, FsModuleLoader, ModuleSpecifier};
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_node::NodeExtInitServices;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::{MainWorker, WorkerOptions, WorkerServiceOptions};
use deno_runtime::{BootstrapOptions, WorkerExecutionMode, WorkerLogLevel};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use deno_error::JsErrorBox;
use deno_features::FeatureChecker;
use deno_lib::args::{get_root_cert_store, CaData, RootCertStoreLoadError};
use deno_lib::npm::{create_npm_process_state_provider, NpmRegistryReadPermissionChecker, NpmRegistryReadPermissionCheckerMode};
use deno_lib::worker::{LibMainWorkerFactory, LibMainWorkerOptions, StorageKeyResolver};
use deno_resolver::cjs::{CjsTracker, IsCjsResolutionMode};
use deno_runtime::deno_telemetry::OtelConfig;
use deno_runtime::deno_tls::RootCertStoreProvider;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_web::BlobStore;
use deno_semver::npm::NpmPackageReqReference;
use node_resolver::{DenoIsBuiltInNodeModuleChecker, NodeResolver, PackageJsonResolver, PackageJsonResolverRc};
use node_resolver::cache::NodeResolutionSys;
use sys_traits;
use sys_traits::impls::RealSys;
use crate::module_loader::UtooModuleLoaderFactory;
// use denort;

deno_core::extension!(
    hello_runtime,
    // ops = [],
    // esm_entry_point = "ext:hello_runtime/bootstrap.js",
    // esm = [dir "examples/extension", "bootstrap.js"]
);

pub fn npm_pkg_req_ref_to_binary_command(
    req_ref: &NpmPackageReqReference,
) -> String {
    req_ref
        .sub_path()
        .map(|s| s.to_string())
        .unwrap_or_else(|| req_ref.req().name.to_string())
}

pub(crate) fn unstable_exit_cb(feature: &str, api_name: &str) {
    log::error!(
    "Unstable API '{api_name}'. The `--unstable-{}` flag must be provided.",
    feature
  );
    deno_runtime::exit(70);
}

struct StandaloneRootCertStoreProvider {
    ca_stores: Option<Vec<String>>,
    ca_data: Option<CaData>,
    cell: OnceLock<Result<RootCertStore, RootCertStoreLoadError>>,
}

impl RootCertStoreProvider for StandaloneRootCertStoreProvider {
    fn get_or_try_init(&self) -> Result<&RootCertStore, JsErrorBox> {
        self
            .cell
            // get_or_try_init was not stable yet when this was written
            .get_or_init(|| {
                get_root_cert_store(None, self.ca_stores.clone(), self.ca_data.clone())
            })
            .as_ref()
            .map_err(|err| JsErrorBox::from_err(err.clone()))
    }
}


pub async fn bootstrap(main_js_path: &str) -> Result<(), AnyError> {
    let js_path = Path::new(main_js_path);
    println!("js_path: {:?}", js_path);
    let main_module = ModuleSpecifier::from_file_path(js_path).unwrap();
    let fs = Arc::new(RealFs);
    let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
        sys_traits::impls::RealSys,
    ));

    let feature_checker = Arc::new({
        let mut checker = FeatureChecker::default();
        checker.set_exit_cb(Box::new(crate::unstable_exit_cb));
        // for feature in metadata.unstable_config.features {
        //     // `metadata` is valid for the whole lifetime of the program, so we
        //     // can leak the string here.
        //     checker.enable_feature(feature.leak());
        // }
        checker
    });
    let sys = RealSys::default();

    // let options = BootstrapOptions {
    //     deno_version: "1.0.0".to_string(),
    //     args: vec![],
    //     cpu_count: 0,
    //     log_level: Default::default(),
    //     enable_op_summary_metrics: false,
    //     enable_testing_features: false,
    //     locale: "".to_string(),
    //     location: None,
    //     no_legacy_abort: false,
    //     // no_color: false,
    //     // is_stdout_tty: false,
    //     // is_stderr_tty: false,
    //     color_level: ColorLevel::None,
    //     unstable_features: vec![],
    //     user_agent: "".to_string(),
    //     inspect: false,
    //     has_node_modules_dir: false,
    //     argv0: None,
    //     node_debug: None,
    //     node_ipc_fd: None,
    //     mode: WorkerExecutionMode::None,
    //     serve_port: None,
    //     serve_host: None,
    //     otel_config: Default::default(),
    //     close_on_idle: false,
    // };

    let lib_main_worker_options = LibMainWorkerOptions {
        argv: vec![],
        log_level: WorkerLogLevel::Info,
        enable_op_summary_metrics: false,
        enable_testing_features: false,
        has_node_modules_dir: false,
        inspect_brk: false,
        inspect_wait: false,
        strace_ops: None,
        is_inspecting: false,
        skip_op_registration: true,
        location: None,
        argv0: NpmPackageReqReference::from_specifier(&main_module)
            .ok()
            .map(|req_ref| npm_pkg_req_ref_to_binary_command(&req_ref))
            .or(std::env::args().next()),
        node_debug: std::env::var("NODE_DEBUG").ok(),
        origin_data_folder_path: None,
        seed: None,
        unsafely_ignore_certificate_errors: None,
        node_ipc: None,
        serve_port: None,
        serve_host: None,
        otel_config: Default::default(),
        // TODO add snapshot here
        startup_snapshot: None,
        no_legacy_abort: false,
        is_standalone: false,
    };

    let pkg_json_resolver = PackageJsonResolverRc::new(PackageJsonResolver::new(
        sys.clone(),
        None,
    ));

    let root_node_modules_dir = std::env::current_dir().unwrap().join("node_modules");
    let node_resolution_sys = NodeResolutionSys::new(sys.clone(), None);
    let in_npm_pkg_checker =
        DenoInNpmPackageChecker::new(CreateInNpmPkgCheckerOptions::Byonm);
    let npm_resolver = NpmResolver::<RealSys>::new::<RealSys>(
        NpmResolverCreateOptions::Byonm(ByonmNpmResolverCreateOptions {
            sys: node_resolution_sys.clone(),
            pkg_json_resolver: pkg_json_resolver.clone(),
            root_node_modules_dir: Some(root_node_modules_dir),
        }),
    );
    // (in_npm_pkg_checker, npm_resolver)

    let node_resolver = Arc::new(NodeResolver::new(
        in_npm_pkg_checker.clone(),
        DenoIsBuiltInNodeModuleChecker,
        npm_resolver.clone(),
        pkg_json_resolver.clone(),
        node_resolution_sys,
        node_resolver::NodeResolverOptions::default(),
    ));
    let root_cert_store_provider = Arc::new(StandaloneRootCertStoreProvider {
        ca_stores: None,
        ca_data: None,
        cell: Default::default(),
    });
    let npm_registry_permission_checker = Arc::new(NpmRegistryReadPermissionChecker::new(sys.clone(), NpmRegistryReadPermissionCheckerMode::Byonm));
    let cjs_tracker = Arc::new(CjsTracker::new(in_npm_pkg_checker.clone(), pkg_json_resolver.clone(), IsCjsResolutionMode::ImplicitTypeCommonJs));
    let module_loader_factory = UtooModuleLoaderFactory::new(
        npm_registry_permission_checker,
        sys.clone(),
        in_npm_pkg_checker.clone(),
        cjs_tracker,
    );

    let main_worker_factory = LibMainWorkerFactory::new(
        // Arc::new(BlobStore::default()),
        // // TODO add code cache here
        // None,
        // feature_checker,
        // fs,
        // None,
        // Box::new(module_loader_factory),
        // node_resolver.clone(),
        // create_npm_process_state_provider(&npm_resolver),
        // pkg_json_resolver,
        // root_cert_store_provider,
        // StorageKeyResolver::empty(),
        // sys.clone(),
        // lib_main_worker_options,


        Arc::new(BlobStore::default()),
        // TODO add code cache here
        None,
        None,
        feature_checker,
        fs,
        None,
        Box::new(module_loader_factory),
        node_resolver.clone(),
        create_npm_process_state_provider(&npm_resolver),
        pkg_json_resolver,
        root_cert_store_provider,
        StorageKeyResolver::empty(),
        sys.clone(),
        lib_main_worker_options,
    );
    let permissions = PermissionsContainer::allow_all(permission_desc_parser);
    let mut worker = main_worker_factory
        .create_main_worker(WorkerExecutionMode::Run, permissions, main_module.clone())?;

    let exit_code = worker.run().await?;
    println!("exit code: {:?}", exit_code);




    // let worker = MainWorker
    // let permissions = PermissionsContainer::allow_all(permission_desc_parser);
    //
    //
    // let mut worker = MainWorker::bootstrap_from_options(
    //     &main_module,
    //     WorkerServiceOptions::<
    //         DenoInNpmPackageChecker,
    //         NpmResolver<sys_traits::impls::RealSys>,
    //         sys_traits::impls::RealSys,
    //     > {
    //         module_loader: Rc::new(FsModuleLoader),
    //         permissions,
    //         blob_store: Default::default(),
    //         broadcast_channel: Default::default(),
    //         feature_checker: Default::default(),
    //         node_services: Default::default(),
    //         // node_services: Some(NodeExtInitServices {
    //         //     node_require_loader,
    //         //     node_resolver: self.node_resolver.clone(),
    //         //     pkg_json_resolver: self.pkg_json_resolver.clone(),
    //         //     sys: self.sys.clone(),
    //         // }),
    //         npm_process_state_provider: Default::default(),
    //         root_cert_store_provider: Default::default(),
    //         fetch_dns_resolver: Default::default(),
    //         shared_array_buffer_store: Default::default(),
    //         compiled_wasm_module_store: Default::default(),
    //         v8_code_cache: Default::default(),
    //         fs,
    //     },
    //     WorkerOptions {
    //         // extensions: vec![hello_runtime::init_ops_and_esm()],
    //         ..Default::default()
    //     },
    // );
    // worker.execute_main_module(&main_module).await?;
    // worker.run_event_loop(false).await?;
    Ok(())
    // let worker = MainWorker::bootstrap_from_options(&url, options);
}

#[cfg(test)]
mod test {
    use crate::bootstrap;

    #[tokio::test]
    async fn my_test() {
        let example_path = std::env::current_dir().unwrap().join("fixtures/index.js");
        let res = bootstrap(example_path.to_str().unwrap()).await;
        println!("res: {:?}", res);
        assert!(true);
    }
}
