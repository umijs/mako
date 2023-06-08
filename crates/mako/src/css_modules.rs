use base64::engine::{general_purpose, Engine};
use swc_css_ast::Stylesheet;
use swc_css_modules::{compile, CssClassName, TransformConfig, TransformResult};
use tracing::warn;

const CSS_MODULES_PATH_SUFFIX: &str = ".module.css";
pub const MAKO_CSS_MODULES_SUFFIX: &str = ".MAKO_CSS_MODULES";

pub struct CssModuleRename {
    pub path: String,
}

impl TransformConfig for CssModuleRename {
    fn new_name_for(&self, local: &swc_atoms::JsWord) -> swc_atoms::JsWord {
        let name = local.to_string();
        let source = format!("{}__{}", &self.path, name);
        let digest = md5::compute(&source);
        let hash = general_purpose::URL_SAFE.encode(digest.0);
        let hash_slice = hash[..8].to_string();
        let new_name = format!("{}-{}", name, hash_slice);
        new_name.into()
    }
}

pub fn is_css_modules_path(path: &str) -> bool {
    path.ends_with(CSS_MODULES_PATH_SUFFIX)
}

pub fn is_mako_css_modules(path: &str) -> bool {
    path.ends_with(MAKO_CSS_MODULES_SUFFIX)
}

pub fn compile_css_modules(path: &str, ast: &mut Stylesheet) -> TransformResult {
    compile(
        ast,
        CssModuleRename {
            path: path.to_string(),
        },
    )
}

pub fn generate_code_for_css_modules(path: &str, ast: &mut Stylesheet) -> String {
    let stylesheet = compile_css_modules(path, ast);

    let mut export_names = Vec::new();
    for (name, classes) in stylesheet.renamed.iter() {
        let mut after_transform_classes = Vec::new();
        for v in classes {
            match v {
                CssClassName::Local { name } => {
                    after_transform_classes.push(name.value.to_string());
                }
                CssClassName::Global { name } => {
                    warn!("unspported classname");
                    after_transform_classes.push(name.value.to_string());
                }
                CssClassName::Import { name, from: _ } => {
                    warn!("unspported classname");
                    after_transform_classes.push(name.value.to_string());
                }
            }
        }
        export_names.push((name, after_transform_classes));
    }
    format!(
        r#"
import "{}{}";
export default {{{}}}
"#,
        path,
        MAKO_CSS_MODULES_SUFFIX,
        export_names
            .iter()
            .map(|(name, classes)| format!("\"{}\": `{}`", name, classes.join(" ").trim()))
            .collect::<Vec<String>>()
            .join(",")
    )
}
