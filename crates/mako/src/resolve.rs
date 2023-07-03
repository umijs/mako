use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use anyhow::{anyhow, Result};
use nodejs_resolver::{AliasMap, Options, ResolveResult, Resolver};
use thiserror::Error;
use tracing::debug;

use crate::compiler::Context;
use crate::config::{Config, Platform};
use crate::css_modules::is_mako_css_modules;
use crate::module::{Dependency, ResolveType};

#[derive(Debug, Error)]
enum ResolveError {
    #[error("Resolve {path:?} failed from {from:?}")]
    ResolveError { path: String, from: String },
}

#[derive(Debug, PartialEq)]
enum ResolverType {
    Cjs,
    Esm,
}

pub struct Resolvers {
    cjs: Resolver,
    esm: Resolver,
}

pub fn resolve(
    path: &str,
    dep: &Dependency,
    resolvers: &Resolvers,
    context: &Arc<Context>,
) -> Result<(String, Option<String>)> {
    let resolver = if dep.resolve_type == ResolveType::Require {
        &resolvers.cjs
    } else {
        &resolvers.esm
    };
    do_resolve(path, &dep.source, resolver, Some(&context.config.externals))
}

// TODO:
// - 支持物理缓存，让第二次更快
fn do_resolve(
    path: &str,
    source: &str,
    resolver: &Resolver,
    externals: Option<&HashMap<String, String>>,
) -> Result<(String, Option<String>)> {
    let external = if let Some(externals) = externals {
        externals.get(&source.to_string()).cloned()
    } else {
        None
    };
    if let Some(external) = external {
        Ok((source.to_string(), Some(external)))
    } else if is_mako_css_modules(source) {
        // css_modules has resolved and mako_css_modules cannot be resolved since the suffix
        Ok((source.to_string(), None))
    } else {
        let path = PathBuf::from(path);
        // 所有的 path 都是文件，所以 parent() 肯定是其所在目录
        let parent = path.parent().unwrap();
        debug!("parent: {:?}, source: {:?}", parent, source);
        let result = resolver.resolve(parent, source);
        if let Ok(ResolveResult::Resource(resource)) = result {
            let path = resource.path.to_string_lossy().to_string();
            Ok((path, None))
        } else {
            Err(anyhow!(ResolveError::ResolveError {
                path: source.to_string(),
                from: path.to_string_lossy().to_string(),
            }))
        }
    }
}

pub fn get_resolvers(config: &Config) -> Resolvers {
    let cjs_resolver = get_resolver(config, ResolverType::Cjs);
    let esm_resolver = get_resolver(config, ResolverType::Esm);
    Resolvers {
        cjs: cjs_resolver,
        esm: esm_resolver,
    }
}

fn get_resolver(config: &Config, resolver_type: ResolverType) -> Resolver {
    let alias = parse_alias(config.resolve.alias.clone());
    let is_browser = config.platform == Platform::Browser;
    // TODO: read from config
    Resolver::new(Options {
        alias,
        extensions: vec![
            ".js".to_string(),
            ".jsx".to_string(),
            ".ts".to_string(),
            ".tsx".to_string(),
            ".mjs".to_string(),
            ".cjs".to_string(),
        ],
        condition_names: if is_browser {
            if resolver_type == ResolverType::Cjs {
                HashSet::from([
                    "browser".to_string(),
                    "default".to_string(),
                    "require".to_string(),
                ])
            } else {
                // esm
                HashSet::from([
                    "module".to_string(),
                    "browser".to_string(),
                    "import".to_string(),
                    // why add require? e.g. axios needs it
                    "require".to_string(),
                    "default".to_string(),
                ])
            }
        } else {
            HashSet::from([
                "module".to_string(),
                "node".to_string(),
                "import".to_string(),
                "default".to_string(),
                "require".to_string(),
            ])
        },
        main_fields: if is_browser {
            vec![
                "browser".to_string(),
                "module".to_string(),
                "main".to_string(),
            ]
        } else {
            vec!["module".to_string(), "main".to_string()]
        },
        ..Default::default()
    })
}

fn parse_alias(alias: HashMap<String, String>) -> Vec<(String, Vec<AliasMap>)> {
    let mut result = vec![];
    for (key, value) in alias {
        let alias_map = vec![AliasMap::Target(value)];
        result.push((key, alias_map));
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::Config;
    use crate::resolve::ResolverType;

    #[test]
    fn test_resolve() {
        let x = resolve("test/resolve/normal", None, None, "index.ts", "./source");
        assert_eq!(x, ("source.ts".to_string(), None));
    }

    #[test]
    fn test_resolve_dep() {
        let x = resolve("test/resolve/normal", None, None, "index.ts", "foo");
        assert_eq!(x, ("node_modules/foo/index.js".to_string(), None));
    }

    #[test]
    fn test_resolve_alias() {
        let alias = HashMap::from([("bar".to_string(), "foo".to_string())]);
        let x = resolve(
            "test/resolve/normal",
            Some(alias.clone()),
            None,
            "index.ts",
            "bar",
        );
        assert_eq!(x, ("node_modules/foo/index.js".to_string(), None));
        let x = resolve(
            "test/resolve/normal",
            Some(alias),
            None,
            "index.ts",
            "bar/foo",
        );
        assert_eq!(x, ("node_modules/foo/foo.js".to_string(), None));
    }

    #[test]
    fn test_resolve_externals() {
        let externals = HashMap::from([("react".to_string(), "react".to_string())]);
        let x = resolve(
            "test/resolve/normal",
            None,
            Some(&externals),
            "index.ts",
            "react",
        );
        assert_eq!(x, ("react".to_string(), Some("react".to_string())));
    }

    fn resolve(
        base: &str,
        alias: Option<HashMap<String, String>>,
        externals: Option<&HashMap<String, String>>,
        path: &str,
        source: &str,
    ) -> (String, Option<String>) {
        let current_dir = std::env::current_dir().unwrap();
        let fixture = current_dir.join(base);
        let mut config: Config = Default::default();
        if let Some(alias_config) = alias {
            config.resolve.alias = alias_config;
        }
        let resolver = super::get_resolver(&config, ResolverType::Cjs);
        let (path, external) = super::do_resolve(
            &fixture.join(path).to_string_lossy(),
            source,
            &resolver,
            externals,
        )
        .unwrap();
        println!("> path: {:?}, {:?}", path, external);
        let path = path.replace(format!("{}/", fixture.to_str().unwrap()).as_str(), "");
        (path, external)
    }
}
