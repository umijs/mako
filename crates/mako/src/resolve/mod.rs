use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use cached::proc_macro::cached;
use mako_core::anyhow::{anyhow, Result};
use mako_core::convert_case::{Case, Casing};
use mako_core::regex::{Captures, Regex};
use mako_core::thiserror::Error;
use mako_core::tracing::debug;
use oxc_resolver::{Alias, AliasValue, ResolveError as OxcResolveError, ResolveOptions, Resolver};

mod resource;
pub(crate) use resource::{ExternalResource, ResolvedResource, ResolverResource};

use crate::compiler::Context;
use crate::config::{
    Config, ExternalAdvancedSubpathConverter, ExternalAdvancedSubpathTarget, ExternalConfig,
    Platform,
};
use crate::features::rsc::Rsc;
use crate::module::{Dependency, ResolveType};

#[derive(Debug, Error)]
#[error("Resolve {path:?} failed from {from:?}")]
struct ResolveError {
    path: String,
    from: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ResolverType {
    Cjs,
    Esm,
    Css,
    Ctxt,
}

pub type Resolvers = HashMap<ResolverType, Resolver>;

pub fn resolve(
    path: &str,
    dep: &Dependency,
    resolvers: &Resolvers,
    context: &Arc<Context>,
) -> Result<ResolverResource> {
    mako_core::mako_profile_function!();
    mako_core::mako_profile_scope!("resolve", &dep.source);

    if dep.source.starts_with("virtual:") {
        return Ok(ResolverResource::Virtual(PathBuf::from(&dep.source)));
    }

    let resolver = if parse_path(&dep.source)?.has_query("context") {
        resolvers.get(&ResolverType::Ctxt)
    } else if dep.resolve_type == ResolveType::Require {
        resolvers.get(&ResolverType::Cjs)
    } else if dep.resolve_type == ResolveType::Css {
        resolvers.get(&ResolverType::Css)
    } else {
        resolvers.get(&ResolverType::Esm)
    }
    .unwrap();

    let source = dep.resolve_as.as_ref().unwrap_or(&dep.source);

    do_resolve(path, source, resolver, Some(&context.config.externals))
}

#[cached(key = "String", convert = r#"{ re.to_string() }"#)]
fn create_external_regex(re: &str) -> Regex {
    Regex::new(re).unwrap()
}

fn get_external_target(
    externals: &HashMap<String, ExternalConfig>,
    source: &str,
) -> Option<(String, Option<String>)> {
    let global_obj = "(typeof globalThis !== 'undefined' ? globalThis : self)";

    if let Some(external) = externals.get(source) {
        // handle full match
        // ex. import React from 'react';
        match external {
            ExternalConfig::Basic(external) => Some((
                if external.is_empty() {
                    "''".to_string()
                } else if external.starts_with("commonjs ") {
                    format!("require(\"{}\")", external.replace("commonjs ", ""))
                } else {
                    format!("{}['{}']", global_obj, external)
                },
                None,
            )),
            ExternalConfig::Advanced(config) => Some((
                if config.root.is_empty() {
                    "''".to_string()
                } else if config.module_type.as_ref().is_some_and(|t| t == "commonjs") {
                    format!("require(\"{}\")", config.root)
                } else {
                    format!("{}['{}']", global_obj, config.root)
                },
                config.script.clone(),
            )),
        }
    } else if let Some((advanced_config, subpath_config, subpath)) =
        externals.iter().find_map(|(key, config)| {
            match config {
                ExternalConfig::Advanced(config) if config.subpath.is_some() => {
                    if let Some(caps) = create_external_regex(&format!(
                        r#"(?:^|/node_modules/|[a-zA-Z\d]@){}(/|$)"#,
                        key
                    ))
                    .captures(source)
                    {
                        let subpath = source.split(&caps[0]).collect::<Vec<_>>()[1].to_string();
                        let subpath_config = config.subpath.as_ref().unwrap();

                        match &subpath_config.exclude {
                            // skip if source is excluded
                            Some(exclude)
                                if exclude.iter().any(|e| {
                                    create_external_regex(&format!("(^|/){}(/|$)", e))
                                        .is_match(subpath.as_str())
                                }) =>
                            {
                                None
                            }
                            _ => Some((config, subpath_config, subpath)),
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
    {
        // handle subpath match
        // ex. import Button from 'antd/es/button';
        // find matched subpath rule
        if let Some((rule, caps)) = subpath_config.rules.iter().find_map(|r| {
            let regex = create_external_regex(r.regex.as_str());

            if regex.is_match(subpath.as_str()) {
                Some((r, regex.captures(subpath.as_str()).unwrap()))
            } else {
                None
            }
        }) {
            // generate target from rule target
            match &rule.target {
                // external to empty string
                ExternalAdvancedSubpathTarget::Empty => {
                    Some(("''".to_string(), advanced_config.script.clone()))
                }
                // external to target template
                ExternalAdvancedSubpathTarget::Tpl(target) => {
                    let regex = create_external_regex(r"\$(\d+)");

                    // replace $1, $2, ... with captured groups
                    let mut replaced = regex
                        .replace_all(target, |target_caps: &Captures| {
                            let i = target_caps[1].parse::<usize>().ok().unwrap();

                            caps.get(i).unwrap().as_str().to_string()
                        })
                        .to_string();

                    // convert case if needed
                    // ex. date-picker -> DatePicker
                    if let Some(converter) = &rule.target_converter {
                        replaced = match converter {
                            ExternalAdvancedSubpathConverter::PascalCase => replaced
                                .split('.')
                                .map(|s| s.to_case(Case::Pascal))
                                .collect::<Vec<_>>()
                                .join("."),
                        };
                    }
                    Some((
                        format!("{}['{}'].{}", global_obj, advanced_config.root, replaced),
                        advanced_config.script.clone(),
                    ))
                }
            }
        } else {
            None
        }
    } else {
        None
    }
}

// TODO:
// - 支持物理缓存，让第二次更快
fn do_resolve(
    path: &str,
    source: &str,
    resolver: &Resolver,
    externals: Option<&HashMap<String, ExternalConfig>>,
) -> Result<ResolverResource> {
    let external = if let Some(externals) = externals {
        get_external_target(externals, source)
    } else {
        None
    };
    if let Some((external, script)) = external {
        Ok(ResolverResource::External(ExternalResource {
            source: source.to_string(),
            external,
            script,
        }))
    } else {
        let path = PathBuf::from(path);
        // 所有的 path 都是文件，所以 parent() 肯定是其所在目录
        let parent = path.parent().unwrap();
        debug!("parent: {:?}, source: {:?}", parent, source);
        let result = resolver.resolve(parent, source);
        match result {
            Ok(resolution) => {
                // TODO: 只在 watch 时且二次编译时才做这个检查
                // TODO: 临时方案，需要改成删除文件时删 resolve cache 里的内容
                // 比如把 util.ts 改名为 util.tsx，目前应该是还有问题的
                if resolution.path().exists() {
                    Ok(ResolverResource::Resolved(ResolvedResource(resolution)))
                } else {
                    Err(anyhow!(ResolveError {
                        path: source.to_string(),
                        from: path.to_string_lossy().to_string(),
                    }))
                }
            }
            Err(oxc_resolve_err) => match oxc_resolve_err {
                OxcResolveError::Ignored(path) => {
                    debug!("resolve ignored: {:?}", source);
                    Ok(ResolverResource::Ignored(path))
                }
                _ => {
                    eprintln!(
                        "failed to resolve {} from {} with resolver err: {:?}",
                        source,
                        path.to_string_lossy(),
                        oxc_resolve_err
                    );
                    Err(anyhow!(ResolveError {
                        path: source.to_string(),
                        from: path.to_string_lossy().to_string(),
                    }))
                }
            },
        }
    }
}

pub fn get_resolvers(config: &Config) -> Resolvers {
    let cjs_resolver = get_resolver(config, ResolverType::Cjs);
    let esm_resolver = get_resolver(config, ResolverType::Esm);
    let css_resolver = get_resolver(config, ResolverType::Css);
    let ctxt_resolver = get_resolver(config, ResolverType::Ctxt);

    let mut resolvers = HashMap::new();
    resolvers.insert(ResolverType::Cjs, cjs_resolver);
    resolvers.insert(ResolverType::Esm, esm_resolver);
    resolvers.insert(ResolverType::Css, css_resolver);
    resolvers.insert(ResolverType::Ctxt, ctxt_resolver);

    resolvers
}

pub fn get_module_extensions() -> Vec<String> {
    vec![
        ".js".to_string(),
        ".jsx".to_string(),
        ".ts".to_string(),
        ".tsx".to_string(),
        ".mjs".to_string(),
        ".cjs".to_string(),
        ".json".to_string(),
    ]
}

fn get_resolver(config: &Config, resolver_type: ResolverType) -> Resolver {
    let alias = parse_alias(config.resolve.alias.clone());
    let is_browser = config.platform == Platform::Browser;
    let extensions = get_module_extensions();
    let options = match (resolver_type, is_browser) {
        (ResolverType::Cjs, true) => ResolveOptions {
            alias,
            extensions,
            condition_names: Rsc::generate_resolve_conditions(
                config,
                vec![
                    "require".to_string(),
                    "module".to_string(),
                    "webpack".to_string(),
                    "browser".to_string(),
                ],
            ),
            main_fields: vec![
                "browser".to_string(),
                "module".to_string(),
                "main".to_string(),
            ],
            alias_fields: vec![vec!["browser".to_string()]],
            ..Default::default()
        },
        (ResolverType::Esm, true) => ResolveOptions {
            alias,
            extensions,
            condition_names: Rsc::generate_resolve_conditions(
                config,
                vec![
                    "import".to_string(),
                    "module".to_string(),
                    "webpack".to_string(),
                    "browser".to_string(),
                ],
            ),
            main_fields: vec![
                "browser".to_string(),
                "module".to_string(),
                "main".to_string(),
            ],
            alias_fields: vec![vec!["browser".to_string()]],
            ..Default::default()
        },
        (ResolverType::Esm, false) => ResolveOptions {
            alias,
            extensions,
            condition_names: Rsc::generate_resolve_conditions(
                config,
                vec![
                    "import".to_string(),
                    "module".to_string(),
                    "webpack".to_string(),
                ],
            ),
            main_fields: vec!["module".to_string(), "main".to_string()],
            ..Default::default()
        },
        (ResolverType::Cjs, false) => ResolveOptions {
            alias,
            extensions,
            condition_names: Rsc::generate_resolve_conditions(
                config,
                vec![
                    "require".to_string(),
                    "module".to_string(),
                    "webpack".to_string(),
                ],
            ),
            main_fields: vec!["module".to_string(), "main".to_string()],
            ..Default::default()
        },
        // css must be browser
        (ResolverType::Css, _) => ResolveOptions {
            extensions: vec![".css".to_string(), ".less".to_string()],
            alias,
            main_fields: vec!["css".to_string(), "style".to_string(), "main".to_string()],
            condition_names: vec!["style".to_string()],
            prefer_relative: true,
            alias_fields: vec![vec!["browser".to_string()]],
            ..Default::default()
        },
        (ResolverType::Ctxt, _) => ResolveOptions {
            alias,
            resolve_to_context: true,
            ..Default::default()
        },
    };

    Resolver::new(options)
}

fn parse_alias(alias: HashMap<String, String>) -> Alias {
    let mut result = vec![];
    for (key, value) in alias {
        let alias_vec = vec![AliasValue::Path(value)];
        result.push((key, alias_vec));
    }
    result
}

pub fn clear_resolver_cache(resolvers: &Resolvers) {
    resolvers
        .iter()
        .for_each(|(_, resolver)| resolver.clear_cache());
}

// TODO: REMOVE THIS, pass file to resolve instead
fn parse_path(path: &str) -> Result<FileRequest> {
    let mut iter = path.split('?');
    let path = iter.next().unwrap();
    let query = iter.next().unwrap_or("");
    let mut query_vec = vec![];
    for pair in query.split('&') {
        if pair.contains('=') {
            let mut it = pair.split('=').take(2);
            let kv = match (it.next(), it.next()) {
                (Some(k), Some(v)) => (k.to_string(), v.to_string()),
                _ => continue,
            };
            query_vec.push(kv);
        } else if !pair.is_empty() {
            query_vec.push((pair.to_string(), "".to_string()));
        }
    }
    Ok(FileRequest {
        path: path.to_string(),
        query: query_vec,
    })
}

#[derive(Debug, Clone)]
pub struct FileRequest {
    pub path: String,
    pub query: Vec<(String, String)>,
}

impl FileRequest {
    pub fn has_query(&self, key: &str) -> bool {
        self.query.iter().any(|(k, _)| *k == key)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{
        Config, ExternalAdvanced, ExternalAdvancedSubpath, ExternalAdvancedSubpathConverter,
        ExternalAdvancedSubpathRule, ExternalAdvancedSubpathTarget, ExternalConfig,
    };
    use crate::resolve::ResolverType;

    #[test]
    fn test_resolve() {
        let x = resolve("test/resolve/normal", None, None, "index.ts", "./source");
        assert_eq!(x, "source.ts".to_string());
    }

    #[test]
    fn test_resolve_dep() {
        let x = resolve("test/resolve/normal", None, None, "index.ts", "foo");
        assert_eq!(x, "node_modules/foo/index.js".to_string());
    }

    #[test]
    fn test_resolve_css() {
        // css resolver should prefer relative module
        let x = css_resolve(
            "test/resolve/css",
            None,
            None,
            "index.css",
            "local/local.css",
        );
        assert_eq!(x, "local/local.css".to_string());

        // css resolver also fallback to node_modules
        let x = css_resolve("test/resolve/css", None, None, "index.css", "dep/dep.css");
        assert_eq!(x, "node_modules/dep/dep.css".to_string());
    }

    #[test]
    fn test_resolve_dep_browser_fields() {
        let x = resolve("test/resolve/browser_fields", None, None, "index.ts", "foo");
        assert_eq!(x, "node_modules/foo/esm-browser.js".to_string());
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
        assert_eq!(x, "node_modules/foo/index.js".to_string());
        let x = resolve(
            "test/resolve/normal",
            Some(alias),
            None,
            "index.ts",
            "bar/foo",
        );
        assert_eq!(x, "node_modules/foo/foo.js".to_string());
    }

    #[test]
    fn test_resolve_externals() {
        let externals = HashMap::from([
            (
                "react".to_string(),
                ExternalConfig::Basic("react".to_string()),
            ),
            ("empty".to_string(), ExternalConfig::Basic("".to_string())),
        ]);
        let x = external_resolve(
            "test/resolve/normal",
            None,
            Some(&externals),
            "index.ts",
            "react",
        );
        assert_eq!(
            x,
            (
                "react".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['react']".to_string()
                ),
                None,
            )
        );
        let x = external_resolve(
            "test/resolve/normal",
            None,
            Some(&externals),
            "index.ts",
            "empty",
        );
        assert_eq!(x, ("empty".to_string(), Some("''".to_string()), None));
    }

    #[test]
    fn test_resolve_advanced_externals() {
        let externals = HashMap::from([
            (
                "antd".to_string(),
                ExternalConfig::Advanced(ExternalAdvanced {
                    root: "antd".to_string(),
                    script: None,
                    module_type: None,
                    subpath: Some(ExternalAdvancedSubpath {
                        exclude: Some(vec!["style".to_string()]),
                        rules: vec![
                            ExternalAdvancedSubpathRule {
                                regex: "/(version|message|notification)$".to_string(),
                                target: ExternalAdvancedSubpathTarget::Tpl("$1".to_string()),
                                target_converter: None,
                            },
                            ExternalAdvancedSubpathRule {
                                regex: "/locales/(.*)$".to_string(),
                                target: ExternalAdvancedSubpathTarget::Empty,
                                target_converter: None,
                            },
                            ExternalAdvancedSubpathRule {
                                regex: "^(?:es|lib)/([a-z-]+)$".to_string(),
                                target: ExternalAdvancedSubpathTarget::Tpl("$1".to_string()),
                                target_converter: Some(
                                    ExternalAdvancedSubpathConverter::PascalCase,
                                ),
                            },
                            ExternalAdvancedSubpathRule {
                                regex: "^(?:es|lib)/([a-z-]+)/([A-Z][a-zA-Z-]+)$".to_string(),
                                target: ExternalAdvancedSubpathTarget::Tpl("$1.$2".to_string()),
                                target_converter: Some(
                                    ExternalAdvancedSubpathConverter::PascalCase,
                                ),
                            },
                        ],
                    }),
                }),
            ),
            (
                "script".to_string(),
                ExternalConfig::Advanced(ExternalAdvanced {
                    root: "ScriptType".to_string(),
                    script: Some("https://example.com/lib/script.js".to_string()),
                    subpath: None,
                    module_type: None,
                }),
            ),
        ]);
        fn internal_resolve(
            externals: &HashMap<String, ExternalConfig>,
            source: &str,
        ) -> (String, Option<String>, Option<String>) {
            external_resolve(
                "test/resolve/externals",
                None,
                Some(externals),
                "index.ts",
                source,
            )
        }
        // expect exclude
        assert_eq!(
            internal_resolve(&externals, "antd/es/button/style"),
            (
                "node_modules/antd/es/button/style/index.js".to_string(),
                None,
                None,
            )
        );
        // expect capture target
        assert_eq!(
            internal_resolve(&externals, "antd/es/version"),
            (
                "antd/es/version".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['antd'].version"
                        .to_string()
                ),
                None,
            )
        );
        // expect empty target
        assert_eq!(
            internal_resolve(&externals, "antd/es/locales/zh_CN"),
            (
                "antd/es/locales/zh_CN".to_string(),
                Some("''".to_string()),
                None
            ),
        );
        // expect target converter
        assert_eq!(
            internal_resolve(&externals, "antd/es/date-picker"),
            (
                "antd/es/date-picker".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['antd'].DatePicker"
                        .to_string()
                ),
                None,
            )
        );
        assert_eq!(
            internal_resolve(&externals, "antd/es/input/Group"),
            (
                "antd/es/input/Group".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['antd'].Input.Group"
                        .to_string()
                ),
                None,
            )
        );
        // expect external absolute path
        assert_eq!(
            // npm mode absolute path
            internal_resolve(&externals, "/path/to/node_modules/antd/es/button"),
            (
                "/path/to/node_modules/antd/es/button".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['antd'].Button"
                        .to_string()
                ),
                None,
            )
        );
        assert_eq!(
            // npminstall mode absolute path
            internal_resolve(
                &externals,
                "/path/to/node_modules/_antd@5.0.0@antd/es/button"
            ),
            (
                "/path/to/node_modules/_antd@5.0.0@antd/es/button".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['antd'].Button"
                        .to_string()
                ),
                None,
            )
        );
        // expect script type external
        assert_eq!(
            internal_resolve(&externals, "script"),
            (
                "script".to_string(),
                Some(
                    "(typeof globalThis !== 'undefined' ? globalThis : self)['ScriptType']"
                        .to_string()
                ),
                Some("https://example.com/lib/script.js".to_string()),
            )
        );
    }

    fn resolve(
        base: &str,
        alias: Option<HashMap<String, String>>,
        externals: Option<&HashMap<String, ExternalConfig>>,
        path: &str,
        source: &str,
    ) -> String {
        base_resolve(base, alias, externals, path, source, ResolverType::Cjs).0
    }

    fn css_resolve(
        base: &str,
        alias: Option<HashMap<String, String>>,
        externals: Option<&HashMap<String, ExternalConfig>>,
        path: &str,
        source: &str,
    ) -> String {
        base_resolve(base, alias, externals, path, source, ResolverType::Css).0
    }

    fn external_resolve(
        base: &str,
        alias: Option<HashMap<String, String>>,
        externals: Option<&HashMap<String, ExternalConfig>>,
        path: &str,
        source: &str,
    ) -> (String, Option<String>, Option<String>) {
        base_resolve(base, alias, externals, path, source, ResolverType::Cjs)
    }

    fn base_resolve(
        base: &str,
        alias: Option<HashMap<String, String>>,
        externals: Option<&HashMap<String, ExternalConfig>>,
        path: &str,
        source: &str,
        resolve_type: ResolverType,
    ) -> (String, Option<String>, Option<String>) {
        let current_dir = std::env::current_dir().unwrap();
        let fixture = current_dir.join(base);
        let mut config: Config = Default::default();
        if let Some(alias_config) = alias {
            config.resolve.alias = alias_config;
        }
        let resolver = super::get_resolver(&config, resolve_type);
        let resource = super::do_resolve(
            &fixture.join(path).to_string_lossy(),
            source,
            &resolver,
            externals,
        )
        .unwrap();
        let path = resource.get_resolved_path();
        let external = resource.get_external();
        let script = resource.get_script();
        println!("> path: {:?}, {:?}", path, external);
        let path = path.replace(format!("{}/", fixture.to_str().unwrap()).as_str(), "");
        (path, external, script)
    }
}
