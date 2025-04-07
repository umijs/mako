use anyhow::{Context, Result};
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Value, Vc};
use turbo_tasks_fs::{FileSystem, FileSystemPath};
use turbopack_core::{
    reference_type::{CommonJsReferenceSubType, ReferenceType},
    resolve::{
        node::node_cjs_resolve_options,
        options::{ImportMap, ImportMapping, ResolvedMap},
        parse::Request,
        pattern::Pattern,
        resolve,
    },
    source::Source,
};
use turbopack_node::execution_context::ExecutionContext;

use crate::{config::Config, mode::Mode};

pub fn mdx_import_source_file() -> RcStr {
    unreachable!()
}

#[turbo_tasks::function]
pub async fn get_postcss_package_mapping(
    project_path: ResolvedVc<FileSystemPath>,
) -> Result<Vc<ImportMapping>> {
    Ok(
        ImportMapping::Alternatives(vec![ImportMapping::PrimaryAlternative(
            "postcss".into(),
            Some(project_path),
        )
        .resolved_cell()])
        .cell(),
    )
}

/// Computes the  client fallback import map, which provides
/// polyfills to Node.js externals.
#[turbo_tasks::function]
pub async fn get_client_fallback_import_map() -> Result<Vc<ImportMap>> {
    let mut import_map = ImportMap::empty();

    insert_turbopack_dev_alias(&mut import_map).await?;

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

    Ok(import_map.cell())
}

// Make sure to not add any external requests here.
async fn insert_shared_aliases(
    import_map: &mut ImportMap,
    project_path: ResolvedVc<FileSystemPath>,
    _execution_context: Vc<ExecutionContext>,
    _config: Vc<Config>,
) -> Result<()> {
    let bundler_package = get_bundler_package(*project_path).to_resolved().await?;
    import_map.insert_singleton_alias("@swc/helpers", bundler_package);
    import_map.insert_singleton_alias("styled-jsx", bundler_package);
    import_map.insert_singleton_alias("react", project_path);
    import_map.insert_singleton_alias("react-dom", project_path);

    insert_turbopack_dev_alias(import_map).await?;
    insert_package_alias(
        import_map,
        "@vercel/turbopack-node/",
        turbopack_node::embed_js::embed_fs()
            .root()
            .to_resolved()
            .await?,
    );
    Ok(())
}

/// Inserts an alias to an import mapping into an import map.
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

/// Inserts an alias to @vercel/turbopack-dev into an import map.
async fn insert_turbopack_dev_alias(import_map: &mut ImportMap) -> Result<()> {
    insert_package_alias(
        import_map,
        "@vercel/turbopack-ecmascript-runtime/",
        turbopack_ecmascript_runtime::embed_fs()
            .root()
            .to_resolved()
            .await?,
    );
    Ok(())
}

#[turbo_tasks::function]
pub async fn get_bundler_package(
    context_directory: Vc<FileSystemPath>,
) -> Result<Vc<FileSystemPath>> {
    let result = resolve(
        context_directory,
        Value::new(ReferenceType::CommonJs(CommonJsReferenceSubType::Undefined)),
        Request::parse(Value::new(Pattern::Constant(
            "@utoo/bundler/package.json".into(),
        ))),
        node_cjs_resolve_options(context_directory.root()),
    );
    let source = result
        .first_source()
        .await?
        .context("@utoo/bundler package not found")?;
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
