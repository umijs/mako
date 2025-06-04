use std::collections::BTreeMap;

use anyhow::{Context, Result};
use rustc_hash::FxHashMap;
use turbo_rcstr::RcStr;
use turbo_tasks::{FxIndexMap, ResolvedVc, Value, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::{
    reference_type::{CommonJsReferenceSubType, ReferenceType},
    resolve::{
        node::node_cjs_resolve_options,
        options::{ConditionValue, ImportMap, ImportMapping, ResolvedMap},
        parse::Request,
        pattern::Pattern,
        resolve, ExternalTraced, ExternalType, ResolveAliasMap, SubpathValue,
    },
    source::Source,
};
use turbopack_node::execution_context::ExecutionContext;

use crate::{config::Config, mode::Mode};

pub fn mdx_import_source_file() -> RcStr {
    unreachable!()
}

#[turbo_tasks::function]
pub async fn get_postcss_package_mapping() -> Result<Vc<ImportMapping>> {
    Ok(
        ImportMapping::Alternatives(vec![ImportMapping::PrimaryAlternative(
            "postcss".into(),
            None,
        )
        .resolved_cell()])
        .cell(),
    )
}

/// Computes the  client fallback import map, which provides
/// polyfills to Node.js externals.
#[turbo_tasks::function]
pub async fn get_client_fallback_import_map() -> Result<Vc<ImportMap>> {
    let import_map = ImportMap::empty();

    // insert_package_alias(
    //     &mut import_map,
    //     "@utoo/turbopack-ecmascript-runtime/",
    //     turbopack_ecmascript_runtime::embed_fs()
    //         .root()
    //         .to_resolved()
    //         .await?,
    // );

    Ok(import_map.cell())
}

// Make sure to not add any external requests here.
/// Computes the client import map.
#[turbo_tasks::function]
pub async fn get_client_import_map(
    project_path: ResolvedVc<FileSystemPath>,
    config: Vc<Config>,
    execution_context: Vc<ExecutionContext>,
) -> Result<Vc<ImportMap>> {
    let mut import_map = ImportMap::empty();

    insert_shared_aliases(&mut import_map, project_path, execution_context, config).await?;

    insert_alias_option(
        &mut import_map,
        project_path,
        config.resolve_alias_options(),
        ["browser"],
    )
    .await?;

    Ok(import_map.cell())
}

// Make sure to not add any external requests here.
async fn insert_shared_aliases(
    import_map: &mut ImportMap,
    project_path: ResolvedVc<FileSystemPath>,
    _execution_context: Vc<ExecutionContext>,
    _config: Vc<Config>,
) -> Result<()> {
    // let pack_package = get_pack_package(*project_path).to_resolved().await?;
    // import_map.insert_singleton_alias("@swc/helpers", pack_package);
    // import_map.insert_singleton_alias("styled-jsx", pack_package);
    import_map.insert_singleton_alias("react", project_path);
    import_map.insert_singleton_alias("react-dom", project_path);

    // insert_package_alias(
    //     import_map,
    //     "@utoo/turbopack-ecmascript-runtime/",
    //     turbopack_ecmascript_runtime::embed_fs()
    //         .root()
    //         .to_resolved()
    //         .await?,
    // );
    // insert_package_alias(
    //     import_map,
    //     "@utoo/turbopack-node/",
    //     turbopack_node::embed_js::embed_fs()
    //         .root()
    //         .to_resolved()
    //         .await?,
    // );

    Ok(())
}

pub async fn insert_alias_option<const N: usize>(
    import_map: &mut ImportMap,
    project_path: ResolvedVc<FileSystemPath>,
    alias_options: Vc<ResolveAliasMap>,
    conditions: [&'static str; N],
) -> Result<()> {
    let conditions = BTreeMap::from(conditions.map(|c| (c.into(), ConditionValue::Set)));
    for (alias, value) in &alias_options.await? {
        if let Some(mapping) = export_value_to_import_mapping(value, &conditions, project_path) {
            import_map.insert_alias(alias, mapping);
        }
    }
    Ok(())
}

fn export_value_to_import_mapping(
    value: &SubpathValue,
    conditions: &BTreeMap<RcStr, ConditionValue>,
    project_path: ResolvedVc<FileSystemPath>,
) -> Option<ResolvedVc<ImportMapping>> {
    let mut result = Vec::new();
    value.add_results(
        conditions,
        &ConditionValue::Unset,
        &mut FxHashMap::default(),
        &mut result,
    );
    if result.is_empty() {
        None
    } else {
        Some(if result.len() == 1 {
            ImportMapping::PrimaryAlternative(result[0].0.into(), Some(project_path))
                .resolved_cell()
        } else {
            ImportMapping::Alternatives(
                result
                    .iter()
                    .map(|(m, _)| {
                        ImportMapping::PrimaryAlternative((*m).into(), Some(project_path))
                            .resolved_cell()
                    })
                    .collect(),
            )
            .resolved_cell()
        })
    }
}

#[allow(dead_code)]
fn insert_exact_alias_map(
    import_map: &mut ImportMap,
    project_path: ResolvedVc<FileSystemPath>,
    map: FxIndexMap<&'static str, String>,
) {
    for (pattern, request) in map {
        import_map.insert_exact_alias(pattern, request_to_import_mapping(project_path, &request));
    }
}

#[allow(dead_code)]
fn insert_wildcard_alias_map(
    import_map: &mut ImportMap,
    project_path: ResolvedVc<FileSystemPath>,
    map: FxIndexMap<&'static str, String>,
) {
    for (pattern, request) in map {
        import_map
            .insert_wildcard_alias(pattern, request_to_import_mapping(project_path, &request));
    }
}

/// Inserts an alias to an alternative of import mappings into an import map.
#[allow(dead_code)]
fn insert_alias_to_alternatives<'a>(
    import_map: &mut ImportMap,
    alias: impl Into<String> + 'a,
    alternatives: Vec<ResolvedVc<ImportMapping>>,
) {
    import_map.insert_exact_alias(
        alias.into(),
        ImportMapping::Alternatives(alternatives).resolved_cell(),
    );
}

/// Inserts an alias to an import mapping into an import map.
#[allow(dead_code)]
fn insert_package_alias(
    import_map: &mut ImportMap,
    prefix: &str,
    package_root: ResolvedVc<FileSystemPath>,
) {
    import_map.insert_wildcard_alias(
        prefix,
        ImportMapping::PrimaryAlternative("./*".into(), Some(package_root)).resolved_cell(),
    );
}

#[turbo_tasks::function]
pub async fn get_pack_package(context_directory: Vc<FileSystemPath>) -> Result<Vc<FileSystemPath>> {
    let result = resolve(
        context_directory,
        Value::new(ReferenceType::CommonJs(CommonJsReferenceSubType::Undefined)),
        Request::parse(Value::new(Pattern::Constant(
            "@utoo/pack/package.json".into(),
        ))),
        node_cjs_resolve_options(context_directory.root()),
    );
    let source = result
        .first_source()
        .await?
        .context("@utoo/pack package not found")?;
    Ok(source.ident().path().parent())
}

pub fn get_client_resolved_map(
    _context: Vc<FileSystemPath>,
    _root: ResolvedVc<FileSystemPath>,
    _mode: Mode,
) -> Vc<ResolvedMap> {
    let glob_mappings = vec![];
    ResolvedMap {
        by_glob: glob_mappings,
    }
    .cell()
}

/// Creates a direct import mapping to the result of resolving a request
/// in a context.
#[allow(dead_code)]
fn request_to_import_mapping(
    context_path: ResolvedVc<FileSystemPath>,
    request: &str,
) -> ResolvedVc<ImportMapping> {
    ImportMapping::PrimaryAlternative(request.into(), Some(context_path)).resolved_cell()
}

/// Creates a direct import mapping to the result of resolving an external
/// request.
#[allow(dead_code)]
fn external_request_to_cjs_import_mapping(
    context_dir: ResolvedVc<FileSystemPath>,
    request: &str,
) -> ResolvedVc<ImportMapping> {
    ImportMapping::PrimaryAlternativeExternal {
        name: Some(request.into()),
        ty: ExternalType::CommonJs,
        traced: ExternalTraced::Traced,
        lookup_dir: context_dir,
    }
    .resolved_cell()
}

/// Creates a direct import mapping to the result of resolving an external
/// request.
#[allow(dead_code)]
fn external_request_to_esm_import_mapping(
    context_dir: ResolvedVc<FileSystemPath>,
    request: &str,
) -> ResolvedVc<ImportMapping> {
    ImportMapping::PrimaryAlternativeExternal {
        name: Some(request.into()),
        ty: ExternalType::EcmaScriptModule,
        traced: ExternalTraced::Traced,
        lookup_dir: context_dir,
    }
    .resolved_cell()
}
