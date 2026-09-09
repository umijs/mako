use anyhow::Result;
use pack_core::mode::Mode;
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ResolvedVc, TurboTasks, Vc};
use turbo_tasks_backend::{BackendOptions, TurboTasksBackend, noop_backing_storage};
use turbo_tasks_fs::{File, FileContent, FileSystem, VirtualFileSystem};
use turbopack_browser::BrowserChunkingContext;
use turbopack_core::{
    asset::AssetContent,
    chunk::{ChunkingContext, ContentHashing},
    environment::{BrowserEnvironment, Environment, ExecutionEnvironment},
    ident::AssetIdent,
    virtual_output::VirtualOutputAsset,
};

async fn chunk_filename(salt: &str, length: u8, extension: &str) -> Result<String> {
    let root = VirtualFileSystem::new_with_name(rcstr!("chunk-hashing"))
        .root()
        .owned()
        .await?;
    let environment = Environment::new(ExecutionEnvironment::Browser(
        BrowserEnvironment {
            dom: true,
            web_worker: false,
            service_worker: false,
            browserslist_query: rcstr!("defaults"),
        }
        .cell()
        .to_resolved()
        .await?,
    ))
    .to_resolved()
    .await?;
    let context = BrowserChunkingContext::builder(
        root.clone(),
        root.clone(),
        rcstr!(""),
        root.clone(),
        root.clone(),
        root.clone(),
        environment,
        Mode::Production.runtime_type(),
    )
    .hash_salt(ResolvedVc::cell(RcStr::from(salt)))
    .chunk_content_hashing(ContentHashing::Direct { length })
    .build();
    let path = root.join("input.js")?;
    let content = AssetContent::file(FileContent::Content(File::from("/* chunk */")).cell())
        .to_resolved()
        .await?;
    let asset = VirtualOutputAsset::new(path.clone(), *content);
    let output = context
        .chunk_path(
            Some(Vc::upcast(asset)),
            AssetIdent::from_path(path).into_vc(),
            Some(rcstr!("worker")),
            extension.into(),
        )
        .await?;
    Ok(output.path.to_string())
}

#[tokio::test]
async fn default_chunk_hash_respects_salt_and_length() -> Result<()> {
    let tt = TurboTasks::new(TurboTasksBackend::new(
        BackendOptions {
            storage_mode: None,
            ..Default::default()
        },
        noop_backing_storage(),
    ));
    tt.run_once(async {
        check_chunk_hashing().read_strongly_consistent().await?;
        Ok(())
    })
    .await
}

#[turbo_tasks::function(operation, root)]
async fn check_chunk_hashing() -> Result<Vc<()>> {
    for extension in [".js", ".css"] {
        let unsalted = chunk_filename("", 13, extension).await?;
        let salted = chunk_filename("release-a", 13, extension).await?;
        assert_ne!(unsalted, salted, "salt must change the output filename");
        assert_ne!(salted, chunk_filename("release-b", 13, extension).await?);
        assert_eq!(salted, chunk_filename("release-a", 13, extension).await?);
        assert!(salted.starts_with("worker-"));
        assert!(salted.ends_with(extension));
        assert_eq!(salted.len(), "worker-".len() + 13 + extension.len());

        let full = chunk_filename("release-a", 25, extension).await?;
        assert_eq!(full.len(), "worker-".len() + 25 + extension.len());
        for length in [26, u8::MAX] {
            assert_eq!(chunk_filename("release-a", length, extension).await?, full);
        }
    }
    Ok(Vc::cell(()))
}
