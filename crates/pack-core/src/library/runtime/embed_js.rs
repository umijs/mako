use crate::embed_js;
use turbo_rcstr::RcStr;
use turbo_tasks::Vc;
use turbopack_core::{code_builder::Code, context::AssetContext};
use turbopack_ecmascript::StaticEcmascriptCode;

#[turbo_tasks::function]
pub(crate) fn embed_static_code(
    asset_context: Vc<Box<dyn AssetContext>>,
    path: RcStr,
    generate_source_map: bool,
) -> Vc<Code> {
    StaticEcmascriptCode::new(
        asset_context,
        embed_js::embed_file_path(path),
        generate_source_map,
    )
    .code()
}
