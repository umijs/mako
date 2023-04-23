use maplit::hashset;
use nodejs_resolver::{Options, Resolver};
use std::{collections::HashMap, path::PathBuf, vec};

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
                context_path.parent().unwrap().to_path_buf()
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
    let to_resolve = resolve_param.dependency.to_string();

    // support external
    if context.config.externals.contains_key(&to_resolve) {
        return ResolveResult {
            path: to_resolve.clone(),
            is_external: true,
            external_name: Some(context.config.externals.get(&to_resolve).unwrap().clone()),
        };
    }

    // TODO:
    // - alias
    // - [?] folder
    // - [x] node_modules
    // - [x] exports
    // - ...
    // ref: https://github.com/webpack/enhanced-resolve

    let resolver = Resolver::new(Options {
        extensions: vec![
            ".js".to_string(),
            ".jsx".to_string(),
            ".ts".to_string(),
            ".tsx".to_string(),
            ".mjs".to_string(),
        ],
        condition_names: hashset! {
            "node".to_string(),
            "require".to_string(),
            "import".to_string(),
            "default".to_string()
        },
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
                Ok(nodejs_resolver::ResolveResult::Ignored) => panic!("Should not happen dIgnored"),
                Err(_err) => panic!("resolve {} failed", request.display()),
            };
        }
        RequestType::Module { module_id, context } => {
            match resolver.resolve(context.as_path(), module_id.as_str()) {
                Ok(nodejs_resolver::ResolveResult::Resource(resource)) => {
                    return ResolveResult {
                        path: resource.join().to_string_lossy().to_string(),
                        external_name: None,
                        is_external: false,
                    };
                }
                Ok(nodejs_resolver::ResolveResult::Ignored) => panic!("Should not happen dIgnored"),
                Err(_err) => panic!(
                    "Resolve Module {module_id} failed from {}",
                    context.display()
                ),
            };
        }
    }
}
