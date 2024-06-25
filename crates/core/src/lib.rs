#[cfg(feature = "profile")]
pub use eframe;
#[cfg(feature = "profile")]
pub use puffin;
#[cfg(feature = "profile")]
pub use puffin_egui;
pub use swc_core::common as swc_common;
pub use swc_core::css::{
    ast as swc_css_ast, codegen as swc_css_codegen, compat as swc_css_compat,
    minifier as swc_css_minifier, modules as swc_css_modules, parser as swc_css_parser,
    prefixer as swc_css_prefixer, visit as swc_css_visit,
};
pub use swc_core::ecma::transforms::{
    base as swc_ecma_transforms, module as swc_ecma_transforms_modules,
    optimization as swc_ecma_transforms_optimization, proposal as swc_ecma_transforms_proposals,
    react as swc_ecma_transforms_react, typescript as swc_ecma_transforms_typescript,
};
pub use swc_core::ecma::{
    ast as swc_ecma_ast, atoms as swc_atoms, codegen as swc_ecma_codegen,
    minifier as swc_ecma_minifier, parser as swc_ecma_parser, preset_env as swc_ecma_preset_env,
    utils as swc_ecma_utils, visit as swc_ecma_visit,
};
pub use {
    anyhow, base64, cached, clap, colored, config, convert_case, fs_extra, futures, glob, hyper,
    hyper_staticfile_jsutf8, hyper_tungstenite, indexmap, md5, mdxjs, merge_source_map, mime_guess,
    notify, notify_debouncer_full, path_clean, pathdiff, petgraph, rayon, regex, sailfish, serde,
    serde_json, serde_xml_rs, serde_yaml, svgr_rs, swc_emotion, swc_error_reporters,
    swc_node_comments, thiserror, tokio, tokio_tungstenite, toml, tracing, tracing_subscriber,
    tungstenite, twox_hash,
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

#[macro_export]
macro_rules! ternary {
    ($if_condition:expr, $if_stmt:expr, $else_stmt:expr) => {
        if $if_condition {
            $if_stmt
        } else {
            $else_stmt
        }
    };
}
