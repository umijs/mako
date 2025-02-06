use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Debug)]
pub struct EntryItem {
    #[serde(default)]
    pub filename: Option<String>,
    pub import: PathBuf,
}

pub type Entry = BTreeMap<String, EntryItem>;

impl<'de> Deserialize<'de> for EntryItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            Value::String(s) => Ok(EntryItem {
                filename: None,
                import: s.into(),
            }),
            Value::Object(_) => {
                Ok(serde_json::from_value::<EntryItem>(value).map_err(serde::de::Error::custom)?)
            }
            _ => Err(serde::de::Error::custom(format!(
                "invalid `{}` value: {}",
                stringify!(deserialize_umd).replace("deserialize_", ""),
                value
            ))),
        }
    }
}
