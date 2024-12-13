use sailfish::TemplateOnce;

#[derive(TemplateOnce)]
#[template(path = "app_runtime.stpl")]
pub struct AppRuntimeTemplate {
    pub has_dynamic_chunks: bool,
    pub has_hmr: bool,
    pub umd: Option<String>,
    pub umd_export: Vec<String>,
    pub cjs: bool,
    pub pkg_name: Option<String>,
    pub chunk_loading_global: String,
    pub is_browser: bool,
    pub concatenate_enabled: bool,
    pub cross_origin_loading: Option<String>,
    pub global_module_registry: bool,
}
