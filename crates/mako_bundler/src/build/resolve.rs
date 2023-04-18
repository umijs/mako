use std::{collections::HashMap, path::PathBuf};

use relative_path::RelativePath;

use crate::context::Context;

pub struct ResolveParam<'a> {
    pub path: &'a str,
    pub dependency: &'a str,
    pub files: Option<&'a HashMap<String, String>>,
}

pub struct ResolveResult {
    pub path: String,
    pub is_external: bool,
    pub external_name: Option<String>,
}

pub fn resolve(resolve_param: &ResolveParam, context: &Context) -> ResolveResult {
    let mut resolved = resolve_param.dependency.to_string();

    // support external
    if context.config.externals.contains_key(&resolved) {
        return ResolveResult {
            path: resolved.clone(),
            is_external: true,
            external_name: Some(context.config.externals.get(&resolved).unwrap().clone()),
        };
    }

    // TODO:
    // - alias
    // - folder
    // - node_modules
    // - exports
    // - ...
    // ref: https://github.com/webpack/enhanced-resolve

    if resolved.starts_with(".") {
        let path = PathBuf::from(resolve_param.path);
        let mut abs_resolved =
            RelativePath::new(resolve_param.dependency).to_logical_path(path.parent().unwrap());

        //
        if !exists_file(abs_resolved.to_str().unwrap(), resolve_param) {
            // default resolve.extensions
            let default_extensions = ["js", "jsx", "ts", "tsx"];
            for extension in default_extensions {
                let abs_resolved_with_ext = abs_resolved.with_extension(extension);
                // println!(">>> resolve {}", abs_resolved_with_ext.display());
                if exists_file(abs_resolved_with_ext.to_str().unwrap(), resolve_param) {
                    abs_resolved = abs_resolved_with_ext;
                    break;
                }
            }
            if !exists_file(abs_resolved.to_str().unwrap(), resolve_param) {
                panic!(
                    "Dependency {} does not exist, import {} from {}",
                    abs_resolved.display(),
                    resolved,
                    path.parent().unwrap().display()
                );
            }
            resolved = abs_resolved.to_string_lossy().to_string();
        } else {
            resolved = abs_resolved.to_string_lossy().to_string();
        }
    }

    ResolveResult {
        path: resolved,
        is_external: false,
        external_name: None,
    }
}

fn exists_file(path: &str, resolve_param: &ResolveParam) -> bool {
    if resolve_param.files.is_some() {
        return resolve_param.files.as_ref().unwrap().contains_key(path);
    } else {
        let path = PathBuf::from(path);
        path.exists() && path.is_file()
    }
}
