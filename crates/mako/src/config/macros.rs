/**
 * a macro to create deserialize function that allow false value for optional struct
 */
#[macro_export]
macro_rules! create_deserialize_fn {
    ($fn_name:ident, $struct_type:ty) => {
        pub fn $fn_name<'de, D>(deserializer: D) -> Result<Option<$struct_type>, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;

            match value {
                // allow false value for optional struct
                serde_json::Value::Bool(false) => Ok(None),
                // try deserialize
                serde_json::Value::Object(obj) => Ok(Some(
                    serde_json::from_value::<$struct_type>(serde_json::Value::Object(obj))
                        .map_err(serde::de::Error::custom)?,
                )),
                serde_json::Value::String(s) => Ok(Some(
                    serde_json::from_value::<$struct_type>(serde_json::Value::String(s.clone()))
                        .map_err(serde::de::Error::custom)?,
                )),
                _ => Err(serde::de::Error::custom(format!(
                    "invalid `{}` value: {}",
                    stringify!($fn_name).replace("deserialize_", ""),
                    value
                ))),
            }
        }
    };
}
