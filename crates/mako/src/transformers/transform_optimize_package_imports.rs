use std::collections::HashMap;
use std::sync::Arc;

use cached::proc_macro::cached;
use mako_core::anyhow::Result;
use mako_core::nodejs_resolver::DescriptionData;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    ImportDecl, ImportNamedSpecifier, ImportSpecifier, ModuleDecl, ModuleExportName, ModuleItem,
};
use mako_core::swc_ecma_utils::{quote_ident, quote_str};
use mako_core::swc_ecma_visit::Fold;
use mako_core::tracing::debug;

use crate::build::cached_build_module;
use crate::compiler::Context;
use crate::module::{
    Dependency, ExportInfo, ExportSpecifierInfo, ImportSpecifierInfo, ResolveType,
};
use crate::resolve::{resolve, ResolverResource};
use crate::task::{Task, TaskType};

pub fn optimize_package_imports(path: String, context: Arc<Context>) -> impl Fold {
    OptimizePackageImports { path, context }
}

pub fn should_optimize(path: &str, context: Arc<Context>) -> bool {
    let is_under_node_modules = path.contains("node_modules");
    context.config.optimize_package_imports && !is_under_node_modules
}

struct OptimizePackageImports {
    path: String,
    context: Arc<Context>,
}

impl Fold for OptimizePackageImports {
    fn fold_module_items(&mut self, module_items: Vec<ModuleItem>) -> Vec<ModuleItem> {
        debug!("optimize_package_imports: {}", self.path);
        let mut new_module_items = vec![];
        for module_item in module_items {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = &module_item {
                // TODO: performance optimization，可以少一次循环
                // 1. Exclude situations where import source does not need to be replaced:
                //   - namespace import
                //   - named import with non specifier
                // TODO: Consider support import default?
                if import_decl
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.is_namespace() || specifier.is_default())
                {
                    debug!(
                        "  namespace import or default import: {:?}",
                        import_decl.src
                    );
                    new_module_items.push(module_item);
                    continue;
                }

                // specifiers with type ImportNamedSpecifier
                let specifiers = import_decl
                    .specifiers
                    .iter()
                    .map(|specifier| {
                        let specifier = specifier.as_named().unwrap();
                        let imported = if let Some(imported) = &specifier.imported {
                            match imported {
                                ModuleExportName::Ident(ident) => ident.sym.to_string(),
                                ModuleExportName::Str(str) => str.value.to_string(),
                            }
                        } else {
                            specifier.local.sym.to_string()
                        };
                        (imported, specifier)
                    })
                    .collect::<HashMap<_, _>>();
                if specifiers.is_empty() {
                    debug!("  no specifiers: {:?}", import_decl.src);
                    new_module_items.push(module_item);
                    continue;
                }

                // 2. resolve dep, and then determine whether it is a barrel file
                let raw_src = import_decl.src.value.to_string();
                let dep = Dependency {
                    source: raw_src.clone(),
                    resolve_as: None,
                    resolve_type: ResolveType::Import,
                    order: 0,
                    span: None,
                };

                match resolve(&self.path, &dep, &self.context.resolvers, &self.context) {
                    Ok(resolved_resource) => {
                        let x = parse_barrel_file(&resolved_resource, &self.context, false);
                        if let Ok(Some((_, mut export_map))) = x {
                            debug!(
                                "  barrel file: {:?}, export_map: {:?}",
                                import_decl.src.value.to_string(),
                                export_map
                            );
                            // retain export_map if specifiers contains key
                            debug!("    specifier_keys: {:?}", specifiers.keys());
                            export_map.retain(|k, _| specifiers.contains_key(k));
                            debug!("    retain export_map: {:?}", export_map.keys());
                            let path = resolved_resource.get_resolved_path();
                            let stmts = build_import_stmts(&export_map, &specifiers, &path);
                            new_module_items.extend(stmts);
                        } else {
                            debug!("  not barrel file: {:?}", import_decl.src.value.to_string());
                            new_module_items.push(module_item);
                        };
                    }
                    Err(_) => {
                        debug!("  resolve error: {:?}", import_decl.src);
                        new_module_items.push(module_item);
                    }
                }
            } else {
                debug!("  not import decl");
                new_module_items.push(module_item);
            }
        }
        debug!("optimize_package_imports end: {}", self.path);
        new_module_items
    }
}

#[cached(key = "String", convert = r#"{ format!("{:?}", description.dir()) }"#)]
fn has_side_effects(description: &Arc<DescriptionData>) -> bool {
    let pkg_json = description.data().raw();
    if pkg_json.is_object() {
        if let Some(side_effects) = pkg_json.as_object().unwrap().get("sideEffects") {
            match side_effects {
                serde_json::Value::Bool(side_effects) => *side_effects,
                // FIXME:
                // Temporary support antd by this way
                serde_json::Value::Array(side_effects) => !side_effects
                    .iter()
                    .all(|rule| rule.is_string() && rule.as_str().unwrap().ends_with(".css")),
                _ => true,
            }
        } else {
            true
        }
    } else {
        true
    }
}

fn build_import_stmts(
    export_infos: &ExportInfos,
    orig_specifiers: &HashMap<String, &ImportNamedSpecifier>,
    source: &str,
) -> Vec<ModuleItem> {
    let mut stmts = vec![];
    debug!("export_infos: {:?}", export_infos);
    debug!("source: {:?}", source);

    // for origin_specifier
    for (local, specifier) in orig_specifiers {
        debug!("  local: {:?}", local);
        let (source, imported) = if let Some((source, name, _orig)) = export_infos.get(local) {
            debug!("    found: {:?}, {:?}", source, name);
            (
                source.as_str(),
                Some(ModuleExportName::Ident(quote_ident!(name.as_str()))),
            )
        } else {
            debug!("    not found: {:?}", local);
            // TODO:
            // 这里还可以优化下。
            // 现在找不到 export 信息时用原 import 的 source
            // 优化方案是，如果桶文件只有一个 export *，那可以用那个 export * 的 source
            (source, specifier.imported.clone())
        };
        let local_ident = orig_specifiers.get(local).as_ref().unwrap().local.clone();
        let specifiers = vec![ImportSpecifier::Named(ImportNamedSpecifier {
            span: DUMMY_SP,
            local: local_ident,
            imported,
            is_type_only: false,
        })];
        let import_stmt = ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
            span: DUMMY_SP,
            specifiers,
            src: Box::new(quote_str!(source)),
            type_only: false,
            with: None,
        }));
        debug!("    import_stmt: {:?}", import_stmt);
        stmts.push(import_stmt);
    }
    stmts
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{:?}_{:}", resource.get_resolved_path(), exports_all) }"#
)]
fn parse_barrel_file(
    resource: &ResolverResource,
    context: &Arc<Context>,
    exports_all: bool,
) -> Result<Option<(bool, ExportInfos)>> {
    debug!(
        "  parse_barrel_file: {:?}, exports_all: {:?}",
        resource.get_resolved_path(),
        exports_all
    );
    // only resolved deps is handled
    let resolved = if let ResolverResource::Resolved(resolved) = &resource {
        Some(resolved)
    } else {
        None
    };
    if resolved.is_none() {
        debug!("    not resolved");
        return Ok(None);
    }
    let resolved = resolved.unwrap();
    // handle side effects
    let side_effects = if let Some(description) = &resolved.0.description {
        has_side_effects(description)
    } else {
        true
    };
    debug!("    side_effects: {:?}", side_effects);
    if side_effects {
        debug!("    has side effects");
        return Ok(None);
    }
    // build_module
    let path = resource.get_resolved_path();
    let task = Task::new(TaskType::Normal(path), None);
    let (m, deps, task) = cached_build_module(context, task)?;
    let info = m.info.as_ref().unwrap();
    if !exports_all && !info.is_barrel {
        debug!("    not exports_all && not barrel file: {:?}", task.path);
        return Ok(None);
    }
    // build hash maps
    let mut export_infos = HashMap::new();
    let mut import_infos = HashMap::new();
    info.import_map.iter().for_each(|import| {
        import
            .specifiers
            .iter()
            .for_each(|specifier| match specifier {
                ImportSpecifierInfo::Namespace(n) => {
                    import_infos.insert(n, import.source.clone());
                }
                ImportSpecifierInfo::Named { local, imported: _ } => {
                    import_infos.insert(local, import.source.clone());
                }
                ImportSpecifierInfo::Default(n) => {
                    import_infos.insert(n, import.source.clone());
                }
            });
    });
    let mut resolver_resource_infos = HashMap::new();
    deps.iter().for_each(|dep| {
        resolver_resource_infos.insert(dep.1.source.clone(), dep.0.clone());
    });
    // debug!("6 {:?}", info.export_map);
    let export_map_len = info.export_map.len();
    let is_large_export_map = export_map_len > 20;
    // iter info.export_map
    info.export_map
        .iter()
        .for_each(|export_info| match export_info {
            ExportInfo::Decl { ident } => {
                debug!("    export decl: {:?}", ident);
                export_infos.insert(
                    ident.clone(),
                    ("_".to_string(), ident.clone(), ident.clone()),
                );
            }
            ExportInfo::All { source } => {
                debug!("    export all: {:?}", source);
                if let Some(resolver_resource) = resolver_resource_infos.get(source) {
                    let more_export_infos = parse_barrel_file(resolver_resource, context, true);
                    let path = resolver_resource.get_resolved_path();
                    if let Ok(Some((is_barrel_file, more_export_infos))) = more_export_infos {
                        // (source, orig, exported)
                        for (k, v) in more_export_infos {
                            if is_barrel_file {
                                export_infos.insert(k, (v.0.clone(), v.2.clone(), v.2));
                            } else {
                                export_infos.insert(k, (path.clone(), v.1.clone(), v.1));
                            }
                        }
                    }
                }
            }
            ExportInfo::Named { source, specifiers } => {
                for specifier in specifiers {
                    // export_infos 要以 exported 为 key 进行保存，然后去找依赖里是否有 orig 的值，如果有，就替换掉
                    let (source, exported, orig) = match specifier {
                        // export { x } from 'foo';
                        // export { x as xxx } from 'foo';
                        ExportSpecifierInfo::Named { local, exported } => {
                            let source = if source.is_some() {
                                source.as_ref().unwrap().clone()
                            } else if let Some(source) = import_infos.get(local) {
                                source.to_string()
                            } else {
                                continue;
                            };
                            let exported = exported.as_ref().unwrap_or(local);
                            let exported = exported.to_string();
                            (source, exported, local.clone())
                        }
                        // export default foo;
                        ExportSpecifierInfo::Default(local) => {
                            let source = if let Some(source) = import_infos.get(local) {
                                source.clone()
                            } else {
                                continue;
                            };
                            (source, local.clone(), local.clone())
                        }
                        // export * as foo from 'foo';
                        ExportSpecifierInfo::Namespace(local) => {
                            let source = if source.is_some() {
                                source.as_ref().unwrap().clone()
                            } else if let Some(source) = import_infos.get(local) {
                                source.clone()
                            } else {
                                continue;
                            };
                            (source, local.clone(), local.clone())
                        }
                    };
                    if let Some(resolver_resource) = resolver_resource_infos.get(&source) {
                        // 1、先添加当前依赖
                        // 2、构建和解析依赖文件，如果找到更深的依赖，覆盖到 export_infos 中
                        export_infos.insert(
                            exported.clone(),
                            (
                                resolver_resource.get_resolved_path(),
                                orig.clone(),
                                exported.clone(),
                            ),
                        );
                        // 太大了，比如 @ant-design/icons 有 789 个 export，再找就很慢了
                        if is_large_export_map {
                            continue;
                        }
                        let more_export_infos =
                            parse_barrel_file(resolver_resource, context, exports_all);
                        if let Ok(Some(more_export_infos)) = more_export_infos {
                            if let Some(value) = more_export_infos.1.get(&orig) {
                                export_infos.insert(exported.clone(), value.clone());
                            }
                        }
                    }
                }
            }
        });
    Ok(Some((info.is_barrel, export_infos)))
}

type ExportInfos = HashMap<String, (String, String, String)>;
