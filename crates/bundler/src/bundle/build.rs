use std::{
    env::current_dir,
    mem::forget,
    path::{PathBuf, MAIN_SEPARATOR},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use rustc_hash::FxHashSet;
use turbo_rcstr::RcStr;
use turbo_tasks::{
    apply_effects, ReadConsistency, ResolvedVc, TransientInstance, TryJoinIterExt, TurboTasks,
    Value, Vc,
};
use turbo_tasks_backend::{noop_backing_storage, BackendOptions, TurboTasksBackend};

use turbo_tasks_fs::FileSystem;
use turbopack::{
    evaluate_context::node_build_environment, global_module_ids::get_global_module_id_strategy,
};
use turbopack_browser::BrowserChunkingContext;
use turbopack_core::{
    asset::Asset,
    chunk::{
        availability_info::AvailabilityInfo, ChunkGroupType, ChunkingConfig, ChunkingContext,
        EvaluatableAsset, EvaluatableAssets, MinifyType, SourceMapsType,
    },
    environment::{BrowserEnvironment, Environment, ExecutionEnvironment},
    ident::AssetIdent,
    issue::{handle_issues, IssueReporter, IssueSeverity},
    module::Module,
    module_graph::ModuleGraph,
    output::{OutputAsset, OutputAssets},
    reference::all_assets_from_entries,
    reference_type::{EntryReferenceSubType, ReferenceType},
    resolve::{
        origin::{PlainResolveOrigin, ResolveOriginExt},
        parse::Request,
    },
};
use turbopack_ecmascript_runtime::RuntimeType;
use turbopack_node::execution_context::ExecutionContext;
use turbopack_nodejs::NodeJsChunkingContext;

use crate::{
    arguments::{BuildArguments, Target},
    contexts::{get_client_asset_context, get_client_compile_time_info, NodeEnv},
    env::load_env,
    issue::{ConsoleUi, LogOptions},
    util::{
        normalize_dirs, normalize_entries, output_fs, project_fs, EntryRequest, NormalizedDirs,
    },
};

use super::UtooBundlerBuilder;

impl UtooBundlerBuilder {
    pub fn source_maps_type(mut self, source_maps_type: SourceMapsType) -> Self {
        self.source_maps_type = Some(source_maps_type);
        self
    }

    pub fn minify_type(mut self, minify_type: MinifyType) -> Self {
        self.minify_type = Some(minify_type);
        self
    }

    pub fn target(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }

    pub async fn build(self) -> Result<()> {
        let task = self.turbo_tasks.spawn_once_task::<(), _>(async move {
            let build_result_op = build_internal(
                self.root_dir,
                self.project_dir.clone(),
                self.entry_requests.clone(),
                self.browserslist_query,
                self.source_maps_type.unwrap_or(SourceMapsType::Full),
                self.minify_type
                    .unwrap_or(MinifyType::Minify { mangle: false }),
                self.target.unwrap_or(Target::Browser),
            );

            // Await the result to propagate any errors.
            build_result_op.read_strongly_consistent().await?;

            apply_effects(build_result_op).await?;

            let log_args = TransientInstance::new(LogOptions {
                current_dir: current_dir().unwrap(),
                project_dir: PathBuf::from(self.project_dir.clone()),
                show_all: self.show_all,
                log_detail: self.log_detail,
                log_level: self.log_level,
            });

            let issue_provider = self.issue_reporter.unwrap_or_else(|| {
                // Initialize a ConsoleUi reporter if no custom reporter was provided
                Box::new(move || Vc::upcast(ConsoleUi::new(log_args.clone())))
            });

            handle_issues(
                build_result_op,
                issue_provider.get_issue_reporter(),
                IssueSeverity::Error.into(),
                None,
                None,
            )
            .await?;

            Ok(Default::default())
        });

        self.turbo_tasks
            .wait_task_completion(task, ReadConsistency::Strong)
            .await?;

        Ok(())
    }
}

#[turbo_tasks::function(operation)]
async fn build_internal(
    root_dir: RcStr,
    project_dir: RcStr,
    entry_requests: Vec<EntryRequest>,
    browserslist_query: RcStr,
    source_maps_type: SourceMapsType,
    minify_type: MinifyType,
    target: Target,
) -> Result<Vc<()>> {
    let project_relative = project_dir.strip_prefix(&*root_dir).unwrap();
    let project_relative: RcStr = project_relative
        .strip_prefix(MAIN_SEPARATOR)
        .unwrap_or(project_relative)
        .replace(MAIN_SEPARATOR, "/")
        .into();

    let output_fs = output_fs(project_dir.clone());
    let fs: Vc<Box<dyn FileSystem>> = project_fs(root_dir);
    let root_path = fs.root().to_resolved().await?;
    let project_path = root_path.join(project_relative).to_resolved().await?;

    let build_output_root = output_fs.root().join("dist".into()).to_resolved().await?;

    let build_output_root_to_root_path = project_path
        .join("dist".into())
        .await?
        .get_relative_path_to(&*root_path.await?)
        .context("Project path is in root path")?;

    let node_env = NodeEnv::Production.cell();

    let runtime_type = match *node_env.await? {
        NodeEnv::Development => RuntimeType::Development,
        NodeEnv::Production => RuntimeType::Production,
    };

    let env = load_env();

    let execution_context = ExecutionContext::new(
        *root_path,
        Vc::upcast(
            NodeJsChunkingContext::builder(
                root_path,
                build_output_root,
                ResolvedVc::cell(build_output_root_to_root_path.clone()),
                build_output_root,
                build_output_root,
                build_output_root,
                node_build_environment().to_resolved().await?,
                runtime_type,
            )
            .build(),
        ),
        env,
    );

    let compile_time_info = get_client_compile_time_info(browserslist_query.clone(), node_env);

    let asset_context = get_client_asset_context(
        *project_path,
        execution_context,
        compile_time_info,
        node_env,
        source_maps_type,
    );

    let entry_requests = (*entry_requests
        .into_iter()
        .map(|r| async move {
            Ok(match r {
                EntryRequest::Relative(p) => Request::relative(
                    Value::new(p.clone().into()),
                    Default::default(),
                    Default::default(),
                    false,
                ),
                EntryRequest::Module(m, p) => Request::module(
                    m.clone(),
                    Value::new(p.clone().into()),
                    Default::default(),
                    Default::default(),
                ),
            })
        })
        .try_join()
        .await?)
        .to_vec();

    let origin = PlainResolveOrigin::new(asset_context, project_path.join("_".into()));
    let project_dir = &project_dir;
    let entries = entry_requests
        .into_iter()
        .map(|request_vc| async move {
            let ty = Value::new(ReferenceType::Entry(EntryReferenceSubType::Undefined));

            let request = request_vc.await?;
            origin
                .resolve_asset(request_vc, origin.resolve_options(ty.clone()), ty)
                .await?
                .first_module()
                .await?
                .with_context(|| {
                    format!(
                        "Unable to resolve entry {} from directory {}.",
                        request.request().unwrap(),
                        project_dir
                    )
                })
        })
        .try_join()
        .await?;

    let module_graph = ModuleGraph::from_modules(Vc::cell(vec![(
        entries.clone(),
        match target {
            Target::Browser => ChunkGroupType::Evaluated,
            Target::Node => ChunkGroupType::Entry,
        },
    )]));

    let module_id_strategy = ResolvedVc::upcast(
        get_global_module_id_strategy(module_graph)
            .to_resolved()
            .await?,
    );

    let chunking_context: Vc<Box<dyn ChunkingContext>> = match target {
        Target::Browser => {
            let mut builder = BrowserChunkingContext::builder(
                project_path,
                build_output_root,
                ResolvedVc::cell(build_output_root_to_root_path),
                build_output_root,
                build_output_root,
                build_output_root,
                Environment::new(Value::new(ExecutionEnvironment::Browser(
                    BrowserEnvironment {
                        dom: true,
                        web_worker: false,
                        service_worker: false,
                        browserslist_query: browserslist_query.clone(),
                    }
                    .resolved_cell(),
                )))
                .to_resolved()
                .await?,
                runtime_type,
            )
            .source_maps(source_maps_type)
            .module_id_strategy(module_id_strategy)
            .minify_type(minify_type);

            match *node_env.await? {
                NodeEnv::Development => {}
                NodeEnv::Production => {
                    builder = builder.ecmascript_chunking_config(ChunkingConfig {
                        min_chunk_size: 50_000,
                        max_chunk_count_per_group: 40,
                        max_merge_chunk_size: 200_000,
                        ..Default::default()
                    })
                }
            }

            Vc::upcast(builder.build())
        }
        Target::Node => {
            let mut builder = BrowserChunkingContext::builder(
                project_path,
                build_output_root,
                ResolvedVc::cell(build_output_root_to_root_path),
                build_output_root,
                build_output_root,
                build_output_root,
                Environment::new(Value::new(ExecutionEnvironment::Browser(
                    BrowserEnvironment {
                        dom: true,
                        web_worker: false,
                        service_worker: false,
                        browserslist_query: browserslist_query.clone(),
                    }
                    .resolved_cell(),
                )))
                .to_resolved()
                .await?,
                runtime_type,
            )
            .source_maps(source_maps_type)
            .module_id_strategy(module_id_strategy)
            .minify_type(minify_type);

            match *node_env.await? {
                NodeEnv::Development => {}
                NodeEnv::Production => {
                    builder = builder.ecmascript_chunking_config(ChunkingConfig {
                        min_chunk_size: 50_000,
                        max_chunk_count_per_group: 40,
                        max_merge_chunk_size: 200_000,
                        ..Default::default()
                    })
                }
            }

            Vc::upcast(builder.build())
        }
    };

    let entry_chunk_groups = entries
        .into_iter()
        .map(|entry_module| async move {
            let entry_chunk_groups =
                match ResolvedVc::try_sidecast::<Box<dyn EvaluatableAsset>>(entry_module) {
                    Some(ecmascript) => match target {
                        Target::Browser => {
                            chunking_context
                                .evaluated_chunk_group(
                                    AssetIdent::from_path(
                                        build_output_root
                                            .join(
                                                ecmascript
                                                    .ident()
                                                    .path()
                                                    .file_stem()
                                                    .await?
                                                    .as_deref()
                                                    .unwrap()
                                                    .into(),
                                            )
                                            // TODO: support to config with output.filename
                                            .with_extension("index.js".into()),
                                    ),
                                    EvaluatableAssets::one(*ResolvedVc::upcast(ecmascript)),
                                    module_graph,
                                    Value::new(AvailabilityInfo::Root),
                                )
                                .await?
                                .assets
                        }
                        Target::Node => ResolvedVc::cell(vec![
                            chunking_context
                                .entry_chunk_group(
                                    build_output_root
                                        .join(
                                            ecmascript
                                                .ident()
                                                .path()
                                                .file_stem()
                                                .await?
                                                .as_deref()
                                                .unwrap()
                                                .into(),
                                        )
                                        // TODO: support to config with output.filename
                                        .with_extension("index.js".into()),
                                    EvaluatableAssets::one(*ResolvedVc::upcast(ecmascript)),
                                    module_graph,
                                    OutputAssets::empty(),
                                    Value::new(AvailabilityInfo::Root),
                                )
                                .await?
                                .asset,
                        ]),
                    },
                    _ => {
                        bail!(
                            "Entry module is not chunkable, so it can't be used to bootstrap the \
                         application"
                        )
                    }
                };
            Ok(entry_chunk_groups)
        })
        .try_join()
        .await?;

    let mut chunks: FxHashSet<ResolvedVc<Box<dyn OutputAsset>>> = FxHashSet::default();
    for chunk_group in entry_chunk_groups {
        chunks.extend(&*all_assets_from_entries(*chunk_group).await?);
    }

    chunks
        .iter()
        .map(|c| c.content().write(c.path()))
        .try_join()
        .await?;

    Ok(Default::default())
}

pub async fn build(args: &BuildArguments) -> Result<()> {
    let NormalizedDirs {
        project_dir,
        root_dir,
    } = normalize_dirs(&args.common.dir, &args.common.root)?;

    let tt = TurboTasks::new(TurboTasksBackend::new(
        BackendOptions {
            dependency_tracking: false,
            storage_mode: None,
            ..Default::default()
        },
        noop_backing_storage(),
    ));

    let mut builder = UtooBundlerBuilder::new(tt.clone(), project_dir, root_dir)
        .log_detail(args.common.log_detail)
        .log_level(
            args.common
                .log_level
                .map_or_else(|| IssueSeverity::Warning, |l| l.0),
        )
        .source_maps_type(if args.no_sourcemap {
            SourceMapsType::None
        } else {
            SourceMapsType::Full
        })
        .target(args.common.target.unwrap_or(Target::Node))
        .show_all(args.common.show_all);

    for entry in normalize_entries(&args.common.entries) {
        builder = builder.entry_request(EntryRequest::Relative(entry));
    }

    builder.build().await?;

    if !args.force_memory_cleanup {
        forget(tt);
    }

    Ok(())
}
