use napi::bindgen_prelude::*;
use napi::NapiRaw;
use napi_derive::napi;
use serde_json::Value;

use crate::threadsafe_function::ThreadsafeFunction;

#[napi(object)]
pub struct JsHooks {
    pub name: Option<String>,
    pub enforce: Option<String>,
    #[napi(
        ts_type = "(filePath: string) => Promise<{ content: string, type: 'css'|'js' } | void> | void;"
    )]
    pub load: Option<JsFunction>,
    #[napi(ts_type = "(filePath: string) => Promise<bool> | bool;")]
    pub load_include: Option<JsFunction>,
    #[napi(ts_type = r#"(data: {
    isFirstCompile: boolean;
    time: number;
    stats: {
      hash: number;
      builtAt: number;
      rootPath: string;
      outputPath: string;
      assets: { type: string; name: string; path: string; size: number }[];
      chunkModules: {
        type: string;
        id: string;
        chunks: string[];
        size: number;
      }[];
      modules: Record<
        string,
        { id: string; dependents: string[]; dependencies: string[] }
      >;
      chunks: {
        type: string;
        id: string;
        files: string[];
        entry: boolean;
        modules: { type: string; id: string; size: number; chunks: string[] }[];
        siblings: string[];
        origin: {
          module: string;
          moduleIdentifier: string;
          moduleName: string;
          loc: string;
          request: string;
        }[];
      }[];
      entrypoints: Record<string, { name: string; chunks: string[] }>;
      rscClientComponents: { path; string; moduleId: string }[];
      rscCSSModules: { path; string; moduleId: string; modules: boolean }[];
      startTime: number;
      endTime: number;
    };
  }) => void"#)]
    pub generate_end: Option<JsFunction>,
    #[napi(ts_type = "(path: string, content: Buffer) => Promise<void>;")]
    pub _on_generate_file: Option<JsFunction>,
    #[napi(ts_type = "() => Promise<void>;")]
    pub build_start: Option<JsFunction>,
    #[napi(ts_type = "() => Promise<void>;")]
    pub build_end: Option<JsFunction>,
    #[napi(
        ts_type = "(source: string, importer: string, { isEntry: bool }) => Promise<{ id: string }>;"
    )]
    pub resolve_id: Option<JsFunction>,
    #[napi(
        ts_type = "(content: { content: string, type: 'css' | 'js' }, path: string) => Promise<{ content: string, type: 'css' | 'js' } | void> | void;"
    )]
    pub transform: Option<JsFunction>,
    #[napi(ts_type = "(filePath: string) => Promise<bool> | bool;")]
    pub transform_include: Option<JsFunction>,
}

pub struct TsFnHooks {
    pub build_start: Option<ThreadsafeFunction<(), ()>>,
    pub build_end: Option<ThreadsafeFunction<(), ()>>,
    pub generate_end: Option<ThreadsafeFunction<Value, ()>>,
    pub load: Option<ThreadsafeFunction<String, Option<LoadResult>>>,
    pub load_include: Option<ThreadsafeFunction<String, Option<bool>>>,
    pub resolve_id:
        Option<ThreadsafeFunction<(String, String, ResolveIdParams), Option<ResolveIdResult>>>,
    pub _on_generate_file: Option<ThreadsafeFunction<WriteFile, ()>>,
    pub transform: Option<ThreadsafeFunction<(String, String), Option<TransformResult>>>,
    pub transform_include: Option<ThreadsafeFunction<String, Option<bool>>>,
}

impl TsFnHooks {
    pub fn new(env: Env, hooks: &JsHooks) -> Self {
        Self {
            build_start: hooks.build_start.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            build_end: hooks.build_end.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            generate_end: hooks.generate_end.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            load: hooks.load.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            load_include: hooks.load_include.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            resolve_id: hooks.resolve_id.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            _on_generate_file: hooks._on_generate_file.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            transform: hooks.transform.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
            transform_include: hooks.transform_include.as_ref().map(|hook| unsafe {
                ThreadsafeFunction::from_napi_value(env.raw(), hook.raw()).unwrap()
            }),
        }
    }
}

#[napi(object)]
pub struct WriteFile {
    pub path: String,
    #[napi(ts_type = "Buffer")]
    pub content: Vec<u8>,
}

#[napi(object, use_nullable = true)]
pub struct LoadResult {
    pub content: String,
    #[napi(js_name = "type")]
    pub content_type: String,
}

#[napi(object, use_nullable = true)]
pub struct ResolveIdResult {
    pub id: String,
    pub external: Option<bool>,
}

#[napi(object)]
pub struct ResolveIdParams {
    pub is_entry: bool,
}

#[napi(object, use_nullable = true)]
pub struct TransformResult {
    pub content: String,
    #[napi(js_name = "type")]
    pub content_type: String,
}
