use regex::Regex;
use serde::{Deserialize, Serialize};

use super::generic_usize::GenericUsizeDefault;
use crate::create_deserialize_fn;

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub enum OptimizeAllowChunks {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "entry")]
    Entry,
    #[serde(rename = "async")]
    #[default]
    Async,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct CodeSplitting {
    pub strategy: CodeSplittingStrategy,
    pub options: Option<CodeSplittingStrategyOptions>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum CodeSplittingStrategy {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "granular")]
    Granular,
    #[serde(rename = "advanced")]
    Advanced,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum CodeSplittingStrategyOptions {
    Granular(CodeSplittingGranularOptions),
    Advanced(CodeSplittingAdvancedOptions),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CodeSplittingGranularOptions {
    pub framework_packages: Vec<String>,
    #[serde(default = "GenericUsizeDefault::<160000>::value")]
    pub lib_min_size: usize,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CodeSplittingAdvancedOptions {
    #[serde(default = "GenericUsizeDefault::<20000>::value")]
    pub min_size: usize,
    pub groups: Vec<OptimizeChunkGroup>,
}

impl Default for CodeSplittingAdvancedOptions {
    fn default() -> Self {
        CodeSplittingAdvancedOptions {
            min_size: GenericUsizeDefault::<20000>::value(),
            groups: vec![],
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum OptimizeChunkNameSuffixStrategy {
    #[serde(rename = "packageName")]
    PackageName,
    #[serde(rename = "dependentsHash")]
    DependentsHash,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeChunkGroup {
    pub name: String,
    #[serde(default)]
    pub name_suffix: Option<OptimizeChunkNameSuffixStrategy>,
    #[serde(default)]
    pub allow_chunks: OptimizeAllowChunks,
    #[serde(default = "GenericUsizeDefault::<1>::value")]
    pub min_chunks: usize,
    #[serde(default = "GenericUsizeDefault::<20000>::value")]
    pub min_size: usize,
    #[serde(default = "GenericUsizeDefault::<5000000>::value")]
    pub max_size: usize,
    #[serde(default)]
    pub min_module_size: Option<usize>,
    #[serde(default)]
    pub priority: i8,
    #[serde(default, with = "optimize_test_format")]
    pub test: Option<Regex>,
}

impl Default for OptimizeChunkGroup {
    fn default() -> Self {
        Self {
            allow_chunks: OptimizeAllowChunks::default(),
            min_chunks: GenericUsizeDefault::<1>::value(),
            min_size: GenericUsizeDefault::<20000>::value(),
            max_size: GenericUsizeDefault::<5000000>::value(),
            name: String::default(),
            name_suffix: None,
            min_module_size: None,
            test: None,
            priority: i8::default(),
        }
    }
}

/**
 * custom formatter for convert string to regex
 * @see https://serde.rs/custom-date-format.html
 */
mod optimize_test_format {
    use regex::Regex;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(v: &Option<Regex>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(v) = v {
            serializer.serialize_str(&v.to_string())
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = String::deserialize(deserializer)?;

        if v.is_empty() {
            Ok(None)
        } else {
            Ok(Regex::new(v.as_str()).ok())
        }
    }
}

create_deserialize_fn!(deserialize_code_splitting, CodeSplitting);
