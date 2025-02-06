use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Umd {
    pub name: String,
    #[serde(default)]
    pub export: Vec<String>,
}

pub fn deserialize_umd<'de, D>(deserializer: D) -> Result<Option<Umd>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Object(_) => Ok(Some(
            serde_json::from_value::<Umd>(value).map_err(serde::de::Error::custom)?,
        )),
        serde_json::Value::String(name) => Ok(Some(Umd {
            name: name.clone(),
            ..Default::default()
        })),
        serde_json::Value::Bool(false) => Ok(None),
        _ => Err(serde::de::Error::custom(format!(
            "invalid `{}` value: {}",
            stringify!(deserialize_umd).replace("deserialize_", ""),
            value
        ))),
    }
}
