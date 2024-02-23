use bitflags::bitflags;
use serde::Serialize;

use crate::module::{ImportType, NamedExportType, ResolveType};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
    pub struct Interops: u16 {
        const Default = 1;
        const Wildcard = 1<<2;
        const ExportAll = 1<<3;
    }
}
impl From<&ImportType> for Interops {
    fn from(value: &ImportType) -> Self {
        let mut interops = Interops::empty();

        value.iter().for_each(|x| match x {
            ImportType::Default => {
                interops.insert(Interops::Default);
            }
            ImportType::Namespace => {
                interops.insert(Interops::Wildcard);
            }
            ImportType::Named => {}
            _ => {}
        });
        interops
    }
}

impl From<&NamedExportType> for Interops {
    fn from(value: &NamedExportType) -> Self {
        let mut res = Self::empty();

        value.iter().for_each(|x| match x {
            NamedExportType::Default => {
                res.insert(Interops::Default);
            }
            NamedExportType::Named => {}
            NamedExportType::Namespace => {
                res.insert(Interops::Wildcard);
            }
            _ => {}
        });
        res
    }
}

impl From<&ResolveType> for Interops {
    fn from(value: &ResolveType) -> Self {
        match value {
            ResolveType::Import(import_type) => import_type.into(),
            ResolveType::ExportNamed(named_export_type) => named_export_type.into(),
            ResolveType::ExportAll => Interops::ExportAll,
            ResolveType::Require => Interops::empty(),
            ResolveType::DynamicImport => Interops::empty(),
            ResolveType::Css => Interops::empty(),
            ResolveType::Worker => Interops::empty(),
        }
    }
}

pub struct ConcatenateContext {}
