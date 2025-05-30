use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;
use deno_ast::{MediaType, ModuleKind};
use deno_core::{resolve_import, thiserror, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind};
use deno_core::error::ModuleLoaderError;
use deno_core::futures::FutureExt;
use deno_core::url::Url;
use deno_error::JsErrorBox;
use deno_lib::loader::StrippingTypesNodeModulesError;
use deno_lib::npm::NpmRegistryReadPermissionChecker;
use deno_lib::worker::{CreateModuleLoaderResult, ModuleLoaderFactory};
use deno_resolver::cjs::CjsTracker;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_node::NodeRequireLoader;
use deno_runtime::deno_permissions::PermissionsContainer;
use node_resolver::errors::ClosestPkgJsonError;
use node_resolver::InNpmPackageChecker;
use sys_traits::FsRead;
use sys_traits::impls::RealSys;

struct SharedModuleLoaderState {
    // cjs_tracker: Arc<DenoRtCjsTracker>,
    // code_cache: Option<Arc<DenoCompileCodeCache>>,
    // modules: Arc<StandaloneModules>,
    // node_code_translator: Arc<DenoRtNodeCodeTranslator>,
    // node_resolver: Arc<DenoRtNodeResolver>,
    // npm_module_loader: Arc<DenoRtNpmModuleLoader>,
    npm_registry_permission_checker: Arc<NpmRegistryReadPermissionChecker<RealSys>>,
    sys: RealSys,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    cjs_tracker: Arc<CjsTracker<DenoInNpmPackageChecker, RealSys>>,
    // npm_req_resolver: Arc<DenoRtNpmReqResolver>,
    // vfs: Arc<FileBackedVfs>,
    // workspace_resolver: WorkspaceResolver<DenoRtSys>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(inherit)]
#[error("Failed to load {specifier}")]
pub struct LoadFailedError {
    specifier: ModuleSpecifier,
    #[source]
    #[inherit]
    source: std::io::Error,
}

pub struct FsModuleLoader {
    shared: Arc<SharedModuleLoaderState>,
}

impl ModuleLoader for FsModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dynamic: bool,
        requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let module_specifier = module_specifier.clone();
        let fut = async move {
            let path = module_specifier.to_file_path().map_err(|_| {
                JsErrorBox::generic(format!(
                    "Provided module specifier \"{module_specifier}\" is not a file URL."
                ))
            })?;
            let module_type = if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                // We only return JSON modules if extension was actually `.json`.
                // In other cases we defer to actual requested module type, so runtime
                // can decide what to do with it.
                if ext == "json" {
                    ModuleType::Json
                } else if ext == "wasm" {
                    ModuleType::Wasm
                } else {
                    match &requested_module_type {
                        RequestedModuleType::Other(ty) => ModuleType::Other(ty.clone()),
                        _ => ModuleType::JavaScript,
                    }
                }
            } else {
                ModuleType::JavaScript
            };

            // If we loaded a JSON file, but the "requested_module_type" (that is computed from
            // import attributes) is not JSON we need to fail.
            if module_type == ModuleType::Json
                && requested_module_type != RequestedModuleType::Json
            {
                return Err(ModuleLoaderError::JsonMissingAttribute);
            }

            let code = std::fs::read(path).map_err(|source| {
                JsErrorBox::from_err(LoadFailedError {
                    specifier: module_specifier.clone(),
                    source,
                })
            })?;
            let module = ModuleSource::new(
                module_type,
                ModuleSourceCode::Bytes(code.into_boxed_slice().into()),
                &module_specifier,
                None,
            );
            Ok(module)
        }
            .boxed_local();

        ModuleLoadResponse::Async(fut)
    }
}

impl NodeRequireLoader for FsModuleLoader {
    fn ensure_read_permission<'a>(
        &self,
        permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
        path: &'a std::path::Path,
    ) -> Result<Cow<'a, std::path::Path>, JsErrorBox> {
        // if self.shared.modules.has_file(path) {
        //     // allow reading if the file is in the snapshot
        //     return Ok(Cow::Borrowed(path));
        // }

        self
            .shared
            .npm_registry_permission_checker
            .ensure_read_permission(permissions, path)
            .map_err(JsErrorBox::from_err)
    }

    fn load_text_file_lossy(
        &self,
        path: &std::path::Path,
    ) -> Result<Cow<'static, str>, JsErrorBox> {
        let text = self
            .shared
            .sys
            .fs_read_to_string_lossy(path)
            .map_err(JsErrorBox::from_err)?;
        Ok(text)
        // if media_type.is_emittable() {
        //     let specifier = deno_path_util::url_from_file_path(path)
        //         .map_err(JsErrorBox::from_err)?;
        //     if self.shared.in_npm_pkg_checker.in_npm_package(&specifier) {
        //         return Err(JsErrorBox::from_err(StrippingTypesNodeModulesError {
        //             specifier,
        //         }));
        //     }
        //     self
        //         .emitter
        //         .emit_parsed_source_sync(
        //             &specifier,
        //             media_type,
        //             // this is probably not super accurate due to require esm, but probably ok.
        //             // If we find this causes a lot of churn in the emit cache then we should
        //             // investigate how we can make this better
        //             ModuleKind::Cjs,
        //             &text.into(),
        //         )
        //         .map(Cow::Owned)
        //         .map_err(JsErrorBox::from_err)
        // } else {
        //     Ok(text)
        // }
    }

    fn is_maybe_cjs(&self, specifier: &Url) -> Result<bool, ClosestPkgJsonError> {
        let media_type = MediaType::from_specifier(specifier);
        self.shared.cjs_tracker.is_maybe_cjs(specifier, media_type)
    }
}

pub struct UtooModuleLoaderFactory {
    shared: Arc<SharedModuleLoaderState>,
}

impl UtooModuleLoaderFactory {
    pub fn new(
        npm_registry_permission_checker: Arc<NpmRegistryReadPermissionChecker<RealSys>>,
        sys: RealSys,
        in_npm_pkg_checker: DenoInNpmPackageChecker,
        cjs_tracker: Arc<CjsTracker<DenoInNpmPackageChecker, RealSys>>
    ) -> Self {
        UtooModuleLoaderFactory {
            shared: Arc::new(SharedModuleLoaderState {
                npm_registry_permission_checker,
                sys,
                in_npm_pkg_checker,
                cjs_tracker,
            })
        }
    }

    pub fn create_result(&self) -> CreateModuleLoaderResult {
        let loader = Rc::new(FsModuleLoader {
            shared: self.shared.clone()
        });
        CreateModuleLoaderResult {
            module_loader: loader.clone(),
            node_require_loader: loader,
        }
    }
}

impl ModuleLoaderFactory for UtooModuleLoaderFactory {
    fn create_for_main(
        &self,
        _root_permissions: PermissionsContainer,
    ) -> CreateModuleLoaderResult {
        self.create_result()
    }

    fn create_for_worker(
        &self,
        _parent_permissions: PermissionsContainer,
        _permissions: PermissionsContainer,
    ) -> CreateModuleLoaderResult {
        self.create_result()
    }
}

