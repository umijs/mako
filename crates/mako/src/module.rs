use std::collections::HashSet;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use bitflags::bitflags;
use pathdiff::diff_paths;
use serde::Serialize;
use swc_core::common::{Span, DUMMY_SP};
use swc_core::ecma::ast::{
    BlockStmt, ExportSpecifier, FnExpr, Function, ImportDecl, ImportSpecifier, Module as SwcModule,
    ModuleExportName, NamedExport,
};
use swc_core::ecma::utils::quote_ident;

use crate::ast::css_ast::CssAst;
use crate::ast::file::{win_path, File};
use crate::ast::js_ast::JsAst;
use crate::build::analyze_deps::AnalyzeDepsResult;
use crate::compiler::Context;
use crate::config::ModuleIdStrategy;
use crate::resolve::ResolverResource;

pub type Dependencies = HashSet<Dependency>;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Dependency {
    pub source: String,
    pub resolve_as: Option<String>,
    pub resolve_type: ResolveType,
    pub order: usize,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleSystem {
    CommonJS,
    ESModule,
    Custom,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct ResolveTypeFlags: u16 {
        const Sync  = 1;
        const Async = 1<<2;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct ImportType: u16 {
        const Default = 1;
        const Named = 1<<2;
        const Namespace = 1<<3;
        const SideEffect = 1<<4 ;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct NamedExportType: u16 {
        const Named = 1;
        const Default = 1<<2;
        const Namespace = 1<<3;
    }
}

impl From<&ResolveType> for ResolveTypeFlags {
    fn from(value: &ResolveType) -> Self {
        match value {
            ResolveType::DynamicImport(_) | ResolveType::Worker(_) => Self::Async,
            _ => Self::Sync,
        }
    }
}

impl From<&ImportDecl> for ImportType {
    fn from(decl: &ImportDecl) -> Self {
        if decl.specifiers.is_empty() {
            ImportType::SideEffect
        } else {
            let mut import_type = ImportType::empty();
            for specifier in &decl.specifiers {
                match specifier {
                    ImportSpecifier::Named(_) => {
                        import_type |= ImportType::Named;
                    }
                    ImportSpecifier::Default(_) => {
                        import_type |= ImportType::Default;
                    }
                    ImportSpecifier::Namespace(_) => {
                        import_type |= ImportType::Namespace;
                    }
                }
            }
            import_type
        }
    }
}

impl From<&NamedExportType> for ImportType {
    fn from(value: &NamedExportType) -> Self {
        let mut res = Self::empty();
        value.iter().for_each(|b| match b {
            NamedExportType::Default => res.insert(Self::Default),
            NamedExportType::Named => res.insert(Self::Named),
            NamedExportType::Namespace => res.insert(Self::Namespace),
            _ => {}
        });
        res
    }
}

impl From<&NamedExport> for NamedExportType {
    fn from(decl: &NamedExport) -> Self {
        let mut res = Self::empty();

        decl.specifiers
            .iter()
            .for_each(|specifier| match specifier {
                ExportSpecifier::Namespace(_) => res.insert(Self::Namespace),
                ExportSpecifier::Default(_) => res.insert(Self::Default),
                ExportSpecifier::Named(named) => {
                    if let ModuleExportName::Ident(orig) = &named.orig
                        && orig.sym.eq("default")
                    {
                        res.insert(Self::Default);
                    } else {
                        res.insert(Self::Named);
                    }
                }
            });

        res
    }
}

#[derive(Eq, Hash, PartialEq, Serialize, Debug, Clone, Default)]
pub struct ImportOptions {
    pub chunk_name: Option<String>,
    pub ignore: bool,
}

impl ImportOptions {
    pub fn get_chunk_name(&self) -> &Option<String> {
        &self.chunk_name
    }
}

#[derive(Eq, Hash, PartialEq, Serialize, Debug, Clone)]
pub enum ResolveType {
    Import(ImportType),
    ExportNamed(NamedExportType),
    ExportAll,
    Require,
    DynamicImport(ImportOptions),
    Css,
    Worker(ImportOptions),
}

impl ResolveType {
    pub fn same_enum(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Import(_), Self::Import(_)) => true,
            (_, _) => self == other,
        }
    }
}

impl ResolveType {
    pub fn is_esm(&self) -> bool {
        self.is_sync_esm() || self.is_dynamic_esm()
    }

    pub fn is_sync_esm(&self) -> bool {
        matches!(
            self,
            ResolveType::Import(_) | ResolveType::ExportNamed(_) | ResolveType::ExportAll
        )
    }

    pub fn is_dynamic_esm(&self) -> bool {
        matches!(self, ResolveType::DynamicImport(_))
    }
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub ast: ModuleAst,
    pub file: File,
    pub deps: AnalyzeDepsResult,
    pub external: Option<String>,
    pub raw: String,
    pub raw_hash: u64,
    /// Modules with top-level-await
    pub top_level_await: bool,
    /// The top-level-await module must be an async module, in addition, for example, wasm is also an async module
    /// The purpose of distinguishing top_level_await and is_async is to adapt to runtime_async
    pub is_async: bool,
    pub is_ignored: bool,
    pub resolved_resource: Option<ResolverResource>,
    /// The transformed source map chain of this module
    pub source_map_chain: Vec<Vec<u8>>,
    pub module_system: ModuleSystem,
}

impl Default for ModuleInfo {
    fn default() -> Self {
        Self {
            module_system: ModuleSystem::CommonJS,
            ast: ModuleAst::None,
            file: Default::default(),
            deps: Default::default(),
            external: None,
            raw: "".to_string(),
            raw_hash: 0,
            top_level_await: false,
            is_async: false,
            resolved_resource: None,
            source_map_chain: vec![],
            is_ignored: false,
        }
    }
}

fn md5_hash(source_str: &str, lens: usize) -> String {
    format!("{:x}", md5::compute(source_str))
        .chars()
        .take(lens)
        .collect::<String>()
}

pub fn generate_module_id(origin_module_id: &str, context: &Arc<Context>) -> String {
    match context.config.module_id_strategy {
        ModuleIdStrategy::Hashed => md5_hash(origin_module_id, 8),
        ModuleIdStrategy::Named => {
            // readable ids for debugging usage
            let absolute_path = PathBuf::from(origin_module_id);
            let relative_path = diff_paths(&absolute_path, &context.root).unwrap_or(absolute_path);
            win_path(relative_path.to_str().unwrap())
        }
        ModuleIdStrategy::Numeric => {
            let numeric_ids_map = context.numeric_ids_map.read().unwrap();
            if let Some(numeric_id) = numeric_ids_map.get(origin_module_id) {
                numeric_id.to_string()
            } else {
                md5_hash(origin_module_id, 8)
            }
        }
    }
}

pub fn relative_to_root(module_path: &String, root: &PathBuf) -> String {
    let absolute_path = PathBuf::from(module_path);
    let relative_path = diff_paths(&absolute_path, root).unwrap_or(absolute_path);
    // diff_paths result always starts with ".."/"." or not
    if relative_path.starts_with("..") || relative_path.starts_with(".") {
        relative_path.to_string_lossy().to_string()
    } else {
        PathBuf::from(".")
            .join(relative_path)
            .to_string_lossy()
            .to_string()
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ModuleId {
    pub id: String,
}

impl Ord for ModuleId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for ModuleId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ModuleId {
    // we use absolute path as module id now
    pub fn new(id: String) -> Self {
        Self { id }
    }

    pub fn generate(&self, context: &Arc<Context>) -> String {
        // TODO: 如果是 Hashed 的话，stats 拿不到原始的 chunk_id
        generate_module_id(&self.id, context)
    }

    pub fn from_path(path_buf: PathBuf) -> Self {
        Self {
            id: path_buf.to_string_lossy().to_string(),
        }
    }

    // FIXME: 这里暂时直接通过 module_id 转换为 path，后续如果改了逻辑要记得改
    pub fn to_path(&self) -> PathBuf {
        PathBuf::from(self.id.clone())
    }
}

impl From<String> for ModuleId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl From<&str> for ModuleId {
    fn from(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl From<PathBuf> for ModuleId {
    fn from(path: PathBuf) -> Self {
        Self {
            id: path.to_string_lossy().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModuleAst {
    Script(JsAst),
    Css(CssAst),
    None,
}

impl ModuleAst {
    #[allow(dead_code)]
    pub fn as_script(&self) -> Option<&JsAst> {
        match self {
            ModuleAst::Script(ast) => Some(ast),
            _ => None,
        }
    }

    pub fn script_mut(&mut self) -> Option<&mut JsAst> {
        match self {
            ModuleAst::Script(ast) => Some(ast),
            _ => None,
        }
    }

    pub fn as_script_ast(&self) -> &SwcModule {
        if let Self::Script(script) = self {
            &script.ast
        } else {
            panic!("ModuleAst is not Script")
        }
    }

    pub fn as_script_ast_mut(&mut self) -> &mut SwcModule {
        if let Self::Script(script) = self {
            &mut script.ast
        } else {
            panic!("ModuleAst is not Script")
        }
    }

    pub fn as_css_mut(&mut self) -> &mut CssAst {
        if let Self::Css(css) = self {
            css
        } else {
            panic!("ModuleAst is not Css")
        }
    }

    pub fn as_script_mut(&mut self) -> &mut JsAst {
        if let Self::Script(script) = self {
            script
        } else {
            panic!("ModuleAst is not Css")
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum ModuleType {
    Script,
    Css,
    Raw,
    PlaceHolder,
}

#[derive(Clone)]
pub struct Module {
    pub id: ModuleId,
    pub is_entry: bool,
    pub info: Option<ModuleInfo>,
    pub side_effects: bool,
}

impl Module {
    pub fn new(id: ModuleId, is_entry: bool, info: Option<ModuleInfo>) -> Self {
        Self {
            id,
            is_entry,
            info,
            side_effects: is_entry,
        }
    }

    pub fn as_script(&self) -> Option<&JsAst> {
        self.info.as_ref().and_then(|i| i.ast.as_script())
    }

    pub fn as_mut_script(&mut self) -> Option<&mut JsAst> {
        self.info.as_mut().and_then(|i| i.ast.script_mut())
    }

    pub fn set_info(&mut self, info: Option<ModuleInfo>) {
        self.info = info;
    }

    pub fn is_external(&self) -> bool {
        self.info
            .as_ref()
            .map_or(false, |info| info.external.is_some())
    }

    pub fn is_placeholder(&self) -> bool {
        self.get_module_type() == ModuleType::PlaceHolder
    }

    pub fn get_module_type(&self) -> ModuleType {
        self.info
            .as_ref()
            .map_or(ModuleType::PlaceHolder, |info| match info.ast {
                ModuleAst::Script(_) => ModuleType::Script,
                ModuleAst::Css(_) => ModuleType::Css,
                ModuleAst::None => ModuleType::Raw,
            })
    }

    pub fn get_module_size(&self) -> usize {
        self.info
            .as_ref()
            .map_or(0, |info| info.raw.as_bytes().len())
    }

    // wrap module stmt into a function
    // eg:
    // function(module, exports, require) {
    //   module stmt..
    // }
    pub fn to_module_fn_expr(&self) -> Result<FnExpr> {
        match &self.info.as_ref().unwrap().ast {
            ModuleAst::Script(script) => {
                let mut stmts = Vec::new();

                for n in script.ast.body.iter() {
                    match n.as_stmt() {
                        None => {
                            return Err(anyhow!(
                                "Error: not a stmt found in {:?}, ast: {:?}",
                                self.id.id,
                                n,
                            ));
                        }
                        Some(stmt) => {
                            stmts.push(stmt.clone());
                        }
                    }
                }

                let func = Function {
                    span: DUMMY_SP,
                    ctxt: Default::default(),
                    params: vec![
                        quote_ident!("module").into(),
                        quote_ident!("exports").into(),
                        quote_ident!("__mako_require__").into(),
                    ],
                    decorators: vec![],
                    body: Some(BlockStmt {
                        span: DUMMY_SP,
                        ctxt: Default::default(),
                        stmts,
                    }),
                    is_generator: false,
                    is_async: false,
                    type_params: None,
                    return_type: None,
                };
                Ok(FnExpr {
                    ident: None,
                    function: func.into(),
                })
            }
            // TODO: css modules will be removed in the future
            ModuleAst::Css(_) => Ok(empty_module_fn_expr()),
            ModuleAst::None => Err(anyhow!("ModuleAst::None({}) cannot concert", self.id.id)),
        }
    }
}

impl Debug for Module {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "id={}({:?})", self.id.id, self.get_module_type())
    }
}

fn empty_module_fn_expr() -> FnExpr {
    let func = Function {
        ctxt: Default::default(),
        span: DUMMY_SP,
        params: vec![
            quote_ident!("module").into(),
            quote_ident!("exports").into(),
            quote_ident!("__mako_require__").into(),
        ],
        decorators: vec![],
        body: Some(BlockStmt {
            ctxt: Default::default(),
            span: DUMMY_SP,
            stmts: vec![],
        }),
        is_generator: false,
        is_async: false,
        type_params: None,
        return_type: None,
    };
    FnExpr {
        ident: None,
        function: func.into(),
    }
}
