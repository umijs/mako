use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use mako_core::anyhow::{anyhow, Result};
use mako_core::convert_case::{Case, Casing};
use mako_core::nodejs_resolver::{AliasMap, Options, ResolveResult, Resolver, Resource};
use mako_core::regex::{Captures, Regex};
use mako_core::thiserror::Error;
use mako_core::tracing::debug;

use crate::compiler::Context;
use crate::config::{
    Config, ExternalAdvancedSubpathConverter, ExternalAdvancedSubpathTarget, ExternalConfig,
    Platform,
};
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
    Css,
}

pub struct Resolvers {
    cjs: Resolver,
    esm: Resolver,
    css: Resolver,
}

#[derive(Debug, Clone)]
pub struct ExternalResource {
    pub source: String,
    pub external: String,
    pub script: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedResource(pub Resource);

#[derive(Debug, Clone)]
pub enum ResolverResource {
    External(ExternalResource),
    Resolved(ResolvedResource),
    Ignored,
}

impl ResolverResource {
    pub fn get_resolved_path(&self) -> String {
        match self {
            ResolverResource::External(ExternalResource { source, .. }) => source.to_string(),
            ResolverResource::Resolved(ResolvedResource(resource)) => {
                let mut path = resource.path.to_string_lossy().to_string();
                if resource.query.is_some() {
                    path = format!("{}{}", path, resource.query.as_ref().unwrap());
                }
                path
            }
            ResolverResource::Ignored => "".to_string(),
        }
    }
    pub fn get_external(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { external, .. }) => Some(external.clone()),
            ResolverResource::Resolved(_) => None,
            ResolverResource::Ignored => None,
        }
    }
    pub fn get_script(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { script, .. }) => script.clone(),
            ResolverResource::Resolved(_) => None,
            ResolverResource::Ignored => None,
        }
    }
}

pub fn resolve(
    path: &str,
    dep: &Dependency,
    resolvers: &Resolvers,
    context: &Arc<Context>,
) -> Result<ResolverResource> {
    mako_core::mako_profile_function!();
    mako_core::mako_profile_scope!("resolve", &dep.source);
    let resolver = if dep.resolve_type == ResolveType::Require {
        &resolvers.cjs
    } else if dep.resolve_type == ResolveType::Css {
        &resolvers.css
    } else {
        &resolvers.esm
    };

    let source = dep.resolve_as.as_ref().unwrap_or(&dep.source);

    do_resolve(path, source, resolver, Some(&context.config.externals))
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
                } else {
                    format!("{}.{}", global_obj, external)
                },
                None,
            )),
            ExternalConfig::Advanced(config) => Some((
                format!("{}.{}", global_obj, config.root),
                config.script.clone(),
            )),
        }
    } else if let Some((advanced_config, subpath_config, subpath)) =
        externals.iter().find_map(|(key, config)| {
            match config {
                ExternalConfig::Advanced(config) if config.subpath.is_some() => {
                    if let Some(caps) =
                        Regex::new(&format!(r#"(?:^|/node_modules/|[a-zA-Z\d]@){}(/|$)"#, key))
                            .ok()
                            .unwrap()
                            .captures(source)
                    {
                        let subpath = source.split(&caps[0]).collect::<Vec<_>>()[1].to_string();
                        let subpath_config = config.subpath.as_ref().unwrap();

                        match &subpath_config.exclude {
                            // skip if source is excluded
                            Some(exclude)
                                if exclude.iter().any(|e| {
                                    Regex::new(&format!("(^|/){}(/|$)", e))
                                        .ok()
                                        .unwrap()
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
            let regex = Regex::new(r.regex.as_str()).ok().unwrap();

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
                    let regex = Regex::new(r"\$(\d+)").ok().unwrap();

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
                        format!("{}.{}.{}", global_obj, advanced_config.root, replaced),
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
        if let Ok(result) = result {
            if source.contains("@alipay/knowledge-form") {
                println!("resolve: {:?} -> {:?}", source, result);
            }

            match result {
                ResolveResult::Resource(resource) => {
                    // TODO: 只在 watch 时且二次编译时才做这个检查
                    // TODO: 临时方案，需要改成删除文件时删 resolve cache 里的内容
                    // 比如把 util.ts 改名为 util.tsx，目前应该是还有问题的
                    if resource.path.exists() {
                        Ok(ResolverResource::Resolved(ResolvedResource(resource)))
                    } else {
                        Err(anyhow!(ResolveError::ResolveError {
                            path: source.to_string(),
                            from: path.to_string_lossy().to_string(),
                        }))
                    }
                }
                ResolveResult::Ignored => {
                    debug!("resolve ignored: {:?}", source);
                    Ok(ResolverResource::Ignored)
                }
            }
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
    let css_resolver = get_resolver(config, ResolverType::Css);
    Resolvers {
        cjs: cjs_resolver,
        esm: esm_resolver,
        css: css_resolver,
    }
}

fn get_resolver(config: &Config, resolver_type: ResolverType) -> Resolver {
    let alias = parse_alias(config.resolve.alias.clone());
    let is_browser = config.platform == Platform::Browser;
    let extensions = vec![
        ".js".to_string(),
        ".jsx".to_string(),
        ".ts".to_string(),
        ".tsx".to_string(),
        ".mjs".to_string(),
        ".cjs".to_string(),
        ".json".to_string(),
    ];

    let options = match (resolver_type, is_browser) {
        (ResolverType::Cjs, true) => Options {
            alias,
            extensions,
            condition_names: HashSet::from([
                "require".to_string(),
                "module".to_string(),
                "webpack".to_string(),
                "browser".to_string(),
            ]),
            main_fields: vec![
                "browser".to_string(),
                "module".to_string(),
                "main".to_string(),
            ],
            browser_field: true,
            ..Default::default()
        },
        (ResolverType::Esm, true) => Options {
            alias,
            extensions,
            condition_names: HashSet::from([
                "import".to_string(),
                "module".to_string(),
                "webpack".to_string(),
                "browser".to_string(),
            ]),
            main_fields: vec![
                "browser".to_string(),
                "module".to_string(),
                "main".to_string(),
            ],
            browser_field: true,
            ..Default::default()
        },
        (ResolverType::Esm, false) => Options {
            alias,
            extensions,
            condition_names: HashSet::from([
                "import".to_string(),
                "module".to_string(),
                "webpack".to_string(),
            ]),
            main_fields: vec!["module".to_string(), "main".to_string()],
            ..Default::default()
        },
        (ResolverType::Cjs, false) => Options {
            alias,
            extensions,
            condition_names: HashSet::from([
                "require".to_string(),
                "module".to_string(),
                "webpack".to_string(),
            ]),
            main_fields: vec!["module".to_string(), "main".to_string()],
            ..Default::default()
        },
        // css must be browser
        (ResolverType::Css, _) => Options {
            extensions: vec![".css".to_string(), ".less".to_string()],
            alias,
            main_fields: vec!["css".to_string(), "style".to_string(), "main".to_string()],
            condition_names: HashSet::from(["style".to_string()]),
            prefer_relative: true,
            browser_field: true,
            ..Default::default()
        },
    };

    Resolver::new(options)
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
                Some("(typeof globalThis !== 'undefined' ? globalThis : self).react".to_string()),
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).antd.version"
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).antd.DatePicker"
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).antd.Input.Group"
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).antd.Button"
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).antd.Button"
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
                    "(typeof globalThis !== 'undefined' ? globalThis : self).ScriptType"
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
