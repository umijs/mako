use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum ImportSpecifier {
    // import * as foo from 'foo';
    Namespace(String),
    // import { foo, bar, default as zoo } from 'foo';
    Named {
        local: String,
        imported: Option<String>,
    },
    // import foo from 'foo';
    Default(String),
}

#[derive(Debug, Clone)]
pub enum ExportSpecifier {
    // export * from 'foo';
    All,
    // export { foo, bar, default as zoo } from 'foo';
    Named {
        local: String,
        // "as zoo" is exported
        exported: Option<String>,
    },
    // export default xxx;
    Default,
    // export * as foo from 'foo';
    Namespace(String),
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub source: String,
    pub specifiers: Vec<ImportSpecifier>,
    pub stmt_id: StatementId,
}

#[derive(Debug, Clone)]
pub struct ExportInfo {
    pub source: Option<String>,
    pub specifiers: Vec<ExportSpecifier>,
    pub stmt_id: StatementId,
}

pub type StatementId = usize;

#[derive(Debug, Clone)]
pub struct ImportStatement {
    pub id: StatementId,
    pub info: ImportInfo,
    pub is_self_executed: bool,
    pub defined_ident: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct ExportStatement {
    pub id: StatementId,
    pub info: ExportInfo,
    pub defined_ident: HashSet<String>,
    pub used_ident: HashSet<String>,
}

#[derive(Debug, Clone)]
pub enum StatementType {
    Import(ImportStatement),
    Export(ExportStatement),
    Stmt {
        id: StatementId,
        defined_ident: HashSet<String>,
        used_ident: HashSet<String>,
        is_self_executed: bool,
    },
}

impl StatementType {
    pub fn get_id(&self) -> StatementId {
        match self {
            StatementType::Import(ImportStatement { id, .. }) => *id,
            StatementType::Export(ExportStatement { id, .. }) => *id,
            StatementType::Stmt { id, .. } => *id,
        }
    }

    pub fn get_is_self_executed(&self) -> bool {
        match self {
            StatementType::Import(ImportStatement {
                is_self_executed, ..
            }) => *is_self_executed,
            StatementType::Export(ExportStatement {
                defined_ident: _, ..
            }) => false,
            StatementType::Stmt {
                is_self_executed, ..
            } => *is_self_executed,
        }
    }

    pub fn get_defined_ident(&self) -> &HashSet<String> {
        match self {
            StatementType::Import(ImportStatement { defined_ident, .. }) => defined_ident,
            StatementType::Export(ExportStatement { defined_ident, .. }) => defined_ident,
            StatementType::Stmt { defined_ident, .. } => defined_ident,
        }
    }
    pub fn get_used_ident(&self) -> Option<&HashSet<String>> {
        match self {
            StatementType::Import(ImportStatement { .. }) => None,
            StatementType::Export(ExportStatement { used_ident, .. }) => Option::Some(used_ident),
            StatementType::Stmt { used_ident, .. } => Option::Some(used_ident),
        }
    }
}
