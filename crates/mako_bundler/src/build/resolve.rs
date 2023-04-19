use nodejs_resolver::{AliasMap, Options, Resolver};
use std::{collections::HashMap, path::PathBuf, vec};

use crate::build::resolve;
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

pub enum RequestType {
    Module { module_id: String, context: PathBuf },
    Local { request: PathBuf, context: PathBuf },
}

fn dispatch_request_type(request: &ResolveParam) -> RequestType {
    let context_path = PathBuf::from(request.path);
    if request.dependency.starts_with('.') {
        RequestType::Local {
            context: if context_path.is_dir() {
                context_path
            } else {
                context_path //.parent().unwrap().to_path_buf()
            },
            request: PathBuf::from(request.dependency),
        }
    } else {
        RequestType::Module {
            context: context_path,
            module_id: request.dependency.to_string(),
        }
    }
}

pub fn resolve(resolve_param: &ResolveParam, context: &Context) -> ResolveResult {
    let mut resolved = resolve_param.dependency.to_string();

    dbg!(resolve_param.path, resolve_param.dependency);

    // support external
    if context.config.externals.contains_key(&resolved) {
        return ResolveResult {
            path: resolved.clone(),
            is_external: true,
            external_name: Some(context.config.externals.get(&resolved).unwrap().clone()),
        };
    }

    // dispatch RequestType

    // TODO:
    // - alias
    // - folder
    // - node_modules
    // - exports
    // - ...
    // ref: https://github.com/webpack/enhanced-resolve

    let resolver = Resolver::new(Options {
        extensions: vec![
            ".js".to_string(),
            ".jsx".to_string(),
            ".ts".to_string(),
            ".tsx".to_string(),
        ],
        ..Default::default()
    });
    match dispatch_request_type(resolve_param) {
        RequestType::Local { request, context } => {
            match resolver.resolve(context.as_path(), request.to_str().unwrap()) {
                Ok(nodejs_resolver::ResolveResult::Resource(resource)) => {
                    return ResolveResult {
                        path: resource.join().to_string_lossy().to_string(),
                        external_name: None,
                        is_external: false,
                    };
                }
                Ok(nodejs_resolver::ResolveResult::Ignored) => println!("Ignored"),
                Err(err) => println!("{err:?}"),
            };
        }
        RequestType::Module { module_id, context } => {
            let p = std::env::current_dir().unwrap(); //.unwrap().as_path();

            dbg!(&p);

            match resolver.resolve(context.as_path(), module_id.as_str()) {
                Ok(nodejs_resolver::ResolveResult::Resource(resource)) => {
                    print!("Module ||||");
                    dbg!(resource.join());
                    return ResolveResult {
                        path: resource.join().to_string_lossy().to_string(),
                        external_name: None,
                        is_external: false,
                    };
                }
                Ok(nodejs_resolver::ResolveResult::Ignored) => println!("Ignored"),
                Err(err) => println!("MMMM {err:?}"),
            };
        }
    }

    if resolved.starts_with('.') {
        let path = PathBuf::from(resolve_param.path);
        let mut abs_resolved =
            RelativePath::new(resolve_param.dependency).to_logical_path(path.parent().unwrap());

        //
        if !exists_file(abs_resolved.to_str().unwrap(), resolve_param) {
            // default resolve.extensions
            let default_extensions = &context.config.resolve.extensions;
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
