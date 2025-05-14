use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{Value, ResolvedVc, Vc};
use turbo_tasks_fs::{FileSystemPath, glob::Glob};
use turbopack_core::{
    reference_type::ReferenceType,
    resolve::{
        plugin::{AfterResolvePlugin, AfterResolvePluginCondition},
        ExternalType,
        ExternalTraced,
        ResolveResult,
        ResolveResultItem,
        ResolveResultOption,
        parse::Request,
        pattern::Pattern,
    },
};

/// Mark modules as external follow the config `externals_rules`, 
/// the module which is marked will be resolved at runtime instead of bundled.
#[turbo_tasks::value]
pub struct ExternalModulesResolvePlugin {
    project_path: ResolvedVc<FileSystemPath>,
    root: ResolvedVc<FileSystemPath>,
    externals_rules: ResolvedVc<Vec<RcStr>>
}

#[turbo_tasks::function]
fn condition(root: Vc<FileSystemPath>) -> Vc<AfterResolvePluginCondition> {
    AfterResolvePluginCondition::new(root, Glob::new("**/node_modules/**".into()))
}

#[turbo_tasks::value_impl]
impl AfterResolvePlugin for ExternalModulesResolvePlugin {
    #[turbo_tasks::function]
    fn after_resolve_condition(&self) -> Vc<AfterResolvePluginCondition> {
        condition(*self.root)
    }

    #[turbo_tasks::function]
    async fn after_resolve(
        &self,
        fs_path: ResolvedVc<FileSystemPath>,
        lookup_path: ResolvedVc<FileSystemPath>,
        reference_type: Value<ReferenceType>,
        request: ResolvedVc<Request>,
    ) -> Result<Vc<ResolveResultOption>> {
        let request_value = &*request.await?;
        let Request::Module {
            module: package,
            path: package_subpath,
            ..
        } = request_value
        else {
            return Ok(ResolveResultOption::none());
        };

        let Pattern::Constant(pkg_subpath) = package_subpath else {
            return Ok(ResolveResultOption::none());
        };
        let request_str: RcStr = format!("{package}{package_subpath}").into();
        let externals_rules = &*self.externals_rules.await?;

        #[derive(Debug, Copy, Clone)]
        enum FileType {
            CommonJs,
            EcmaScriptModule,
            UnsupportedExtension,
            InvalidPackageJson,
        }

        async fn get_file_type(
            fs_path: Vc<FileSystemPath>,
            raw_fs_path: &FileSystemPath,
        ) -> Result<FileType> {
            // node.js only supports these file extensions
            // mjs is an esm module and we can't bundle that yet
            let ext = raw_fs_path.extension_ref();
            if matches!(ext, Some("cjs" | "node" | "json")) {
                return Ok(FileType::CommonJs);
            }
            if matches!(ext, Some("mjs")) {
                return Ok(FileType::EcmaScriptModule);
            }
            if matches!(ext, Some("js")) {
                // for .js extension in cjs context, we need to check the actual module type via
                // package.json
                let FindContextFileResult::Found(package_json, _) =
                    *find_context_file(fs_path.parent(), package_json()).await?
                else {
                    // can't find package.json
                    return Ok(FileType::CommonJs);
                };
                let FileJsonContent::Content(package) = &*package_json.read_json().await? else {
                    // can't parse package.json
                    return Ok(FileType::InvalidPackageJson);
                };

                if let Some("module") = package["type"].as_str() {
                    return Ok(FileType::EcmaScriptModule);
                }

                return Ok(FileType::CommonJs);
            }

            Ok(FileType::UnsupportedExtension)
        }


        let mut request = *request;
        let mut request_str = request_str.to_string();



        Ok(ResolveResultOption::some(*ResolveResult::primary(
            ResolveResultItem::External {
                name: request_str.into(),
                ty: ExternalType::Module,
                traced: ExternalTraced::Traced,
            }
        )))

    }
}
