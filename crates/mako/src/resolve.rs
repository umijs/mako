use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    vec,
};

use crate::{compiler::Context, module::Dependency};
use nodejs_resolver::{AliasMap, Options, ResolveResult, Resolver};
use tracing::debug;

pub fn resolve(
    path: &str,
    dep: &Dependency,
    resolver: &Resolver,
    context: &Arc<Context>,
) -> (String, Option<String>) {
    do_resolve(path, &dep.source, resolver, Some(&context.config.externals))
}

// TODO:
// - 支持物理缓存，让第二次更快
// - @swc/helpers 需要内置，而无需用户手动安装 [?]
fn do_resolve(
    path: &str,
    source: &str,
    resolver: &Resolver,
    externals: Option<&HashMap<String, String>>,
) -> (String, Option<String>) {
    let external = if let Some(externals) = externals {
        externals.get(&source.to_string()).cloned()
    } else {
        None
    };
    if let Some(external) = external {
        (source.to_string(), Some(external.to_string()))
    } else {
        let path = PathBuf::from(path);
        // 所有的 path 都是文件，所以 parent() 肯定是其所在目录
        let parent = path.parent().unwrap();
        debug!("parent: {:?}, source: {:?}", parent, source);
        let result = resolver.resolve(parent, &source);
        if let Ok(ResolveResult::Resource(resource)) = result {
            let path = resource.path.to_string_lossy().to_string();
            return (path, None);
        } else {
            panic!(
                "resolve error: {:?}, parent: {:?}, source: {:?}",
                result, parent, source
            );
        }
    }
}

pub fn get_resolver(alias: Option<HashMap<String, String>>) -> Resolver {
    let alias = if let Some(alias) = alias {
        parse_alias(alias)
    } else {
        vec![]
    };
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
        condition_names: HashSet::from([
            "node".to_string(),
            "require".to_string(),
            "import".to_string(),
            "browser".to_string(),
            "default".to_string(),
        ]),
        main_fields: vec!["module".to_string(), "main".to_string()],
        ..Default::default()
    })
}

fn parse_alias(alias: HashMap<String, String>) -> Vec<(String, Vec<AliasMap>)> {
    let mut result = vec![];
    for (key, value) in alias {
        let mut alias_map = vec![];
        alias_map.push(AliasMap::Target(value));
        result.push((key, alias_map));
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
            Some(alias.clone()),
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
        let resolver = super::get_resolver(alias);
        let (path, external) = super::do_resolve(
            &fixture.join(path).to_string_lossy().to_string(),
            source,
            &resolver,
            externals,
        );
        println!("> path: {:?}, {:?}", path, external);
        let path = path
            .replace(format!("{}/", fixture.to_str().unwrap()).as_str(), "")
            .to_string();
        (path, external)
    }
}
