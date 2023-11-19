#[cfg(feature = "profile")]
pub use eframe;
#[cfg(feature = "profile")]
pub use puffin;
#[cfg(feature = "profile")]
pub use puffin_egui;
pub use {
    anyhow, base64, cached, clap, colored, config, convert_case, fs_extra, futures, glob, hyper,
    hyper_staticfile, hyper_tungstenite, indexmap, lazy_static, md5, mdxjs, mime_guess,
    nodejs_resolver, notify, path_clean, pathdiff, petgraph, rayon, regex, serde, serde_json,
    serde_xml_rs, serde_yaml, svgr_rs, swc_atoms, swc_common, swc_css_ast, swc_css_codegen,
    swc_css_compat, swc_css_minifier, swc_css_modules, swc_css_parser, swc_css_prefixer,
    swc_css_visit, swc_ecma_ast, swc_ecma_codegen, swc_ecma_minifier, swc_ecma_parser,
    swc_ecma_preset_env, swc_ecma_transforms, swc_ecma_utils, swc_ecma_visit, swc_emotion,
    swc_error_reporters, swc_node_comments, thiserror, tokio, tokio_tungstenite, toml, tracing,
    tracing_subscriber, tungstenite, twox_hash,
};

#[macro_export]
macro_rules! mako_profile_scope {
    ($id:expr) => {
        #[cfg(feature = "profile")]
        mako_core::puffin::profile_scope!($id);
    };
    ($id:expr, $data:expr) => {
        #[cfg(feature = "profile")]
        mako_core::puffin::profile_scope!($id, $data);
    };
}

#[macro_export]
macro_rules! mako_profile_function {
    () => {
        #[cfg(feature = "profile")]
        mako_core::puffin::profile_function!();
    };
    ($data:expr) => {
        #[cfg(feature = "profile")]
        mako_core::puffin::profile_function!($data);
    };
}
