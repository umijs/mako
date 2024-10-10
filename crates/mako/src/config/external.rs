use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Hash)]
#[serde(untagged)]
pub enum ExternalConfig {
    Basic(String),
    Advanced(ExternalAdvanced),
}

#[derive(Deserialize, Serialize, Debug, Hash)]
pub struct ExternalAdvancedSubpath {
    pub exclude: Option<Vec<String>>,
    pub rules: Vec<ExternalAdvancedSubpathRule>,
}

#[derive(Deserialize, Serialize, Debug, Hash)]
pub struct ExternalAdvanced {
    pub root: String,
    #[serde(rename = "type")]
    pub module_type: Option<String>,
    pub script: Option<String>,
    pub subpath: Option<ExternalAdvancedSubpath>,
}

#[derive(Deserialize, Serialize, Debug, Hash)]
pub struct ExternalAdvancedSubpathRule {
    pub regex: String,
    #[serde(with = "external_target_format")]
    pub target: ExternalAdvancedSubpathTarget,
    #[serde(rename = "targetConverter")]
    pub target_converter: Option<ExternalAdvancedSubpathConverter>,
}

#[derive(Deserialize, Serialize, Debug, Hash)]
pub enum ExternalAdvancedSubpathConverter {
    PascalCase,
}

#[derive(Deserialize, Serialize, Debug, Hash)]
#[serde(untagged)]
pub enum ExternalAdvancedSubpathTarget {
    Empty,
    Tpl(String),
}

/**
 * custom formatter for convert $EMPTY to enum, because rename is not supported for $ symbol
 * @see https://serde.rs/custom-date-format.html
 */
mod external_target_format {
    use serde::{self, Deserialize, Deserializer, Serializer};

    use super::ExternalAdvancedSubpathTarget;

    pub fn serialize<S>(v: &ExternalAdvancedSubpathTarget, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match v {
            ExternalAdvancedSubpathTarget::Empty => serializer.serialize_str("$EMPTY"),
            ExternalAdvancedSubpathTarget::Tpl(s) => serializer.serialize_str(s),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ExternalAdvancedSubpathTarget, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = String::deserialize(deserializer)?;

        if v == "$EMPTY" {
            Ok(ExternalAdvancedSubpathTarget::Empty)
        } else {
            Ok(ExternalAdvancedSubpathTarget::Tpl(v))
        }
    }
}
