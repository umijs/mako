use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Icon {
    Bool(bool),
    Str(String),
    Num(f64),
}

impl Default for Icon {
    fn default() -> Self {
        Icon::Bool(false)
    }
}

// Untagged enums with empty variants (de)serialize in unintuitive ways
// here: https://github.com/serde-rs/serde/issues/1560
macro_rules! named_unit_variant {
    ($variant:ident) => {
        pub mod $variant {
            pub fn serialize<S>(serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let s = stringify!($variant).replace("_", "-");
                serializer.serialize_str(&s)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct V;
                impl<'de> serde::de::Visitor<'de> for V {
                    type Value = ();
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        let mut s = String::new();
                        s.push_str("\"");
                        s.push_str(stringify!($variant).replace("_", "-").as_str());
                        s.push_str("\"");
                        f.write_str(&s)
                    }
                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        let s = stringify!($variant).replace("_", "-");
                        if value == s {
                            Ok(())
                        } else {
                            Err(E::invalid_value(serde::de::Unexpected::Str(value), &self))
                        }
                    }
                }
                deserializer.deserialize_str(V)
            }
        }
    };
}

mod strings {
    named_unit_variant!(start);
    named_unit_variant!(end);
    named_unit_variant!(classic);
    named_unit_variant!(classic_preact);
    named_unit_variant!(automatic);
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum ExpandProps {
    Bool(bool),
    #[serde(with = "strings::start")]
    Start,
    #[serde(with = "strings::end")]
    #[default]
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JSXRuntime {
    #[serde(with = "strings::classic")]
    Classic,
    #[serde(with = "strings::classic_preact")]
    ClassicPreact,
    #[serde(with = "strings::automatic")]
    Automatic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JSXRuntimeImport {
    pub source: String,
    pub namespace: Option<String>,
    pub default_specifier: Option<String>,
    pub specifiers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportType {
    Named,
    Default,
}

/// The options used to transform the SVG.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Setting this to `true` will forward ref to the root SVG tag.
    #[serde(default)]
    #[serde(rename(serialize = "ref"))]
    pub _ref: Option<bool>,

    /// Add title tag via title property.
    /// If title_prop is set to true and no title is provided at render time, this will fallback to an existing title element in the svg if exists.
    #[serde(default)]
    pub title_prop: Option<bool>,

    /// Add desc tag via desc property.
    /// If desc_prop is set to true and no description is provided at render time, this will fallback to an existing desc element in the svg if exists.
    #[serde(default)]
    pub desc_prop: Option<bool>,

    /// All properties given to component will be forwarded on SVG tag.
    /// Possible values: "start", "end" or false.
    #[serde(default)]
    pub expand_props: ExpandProps,

    /// Keep `width` and `height` attributes from the root SVG tag.
    /// Removal is guaranteed if `dimensions: false`, unlike the `remove_dimensions: true` SVGO plugin option which also generates a `viewBox` from the dimensions if no `viewBox` is present.
    #[serde(default)]
    pub dimensions: Option<bool>,

    /// Replace SVG `width` and `height` by a custom value.
    /// If value is omitted, it uses `1em` in order to make SVG size inherits from text size.
    #[serde(default)]
    pub icon: Option<Icon>,

    /// Modify all SVG nodes with uppercase and use a specific template with `react-native-svg` imports.
    /// All unsupported nodes will be removed.
    #[serde(default)]
    pub native: Option<bool>,

    /// Add props to the root SVG tag.
    #[serde(default)]
    // Deserialize object/map while maintaining order
    // here: https://github.com/serde-rs/serde/issues/269
    pub svg_props: Option<LinkedHashMap<String, String>>,

    /// Generates `.tsx` files with TypeScript typings.
    #[serde(default)]
    pub typescript: Option<bool>,

    /// Setting this to `true` will wrap the exported component in `React.memo`.
    #[serde(default)]
    pub memo: Option<bool>,

    /// Replace an attribute value by an other.
    /// The main usage of this option is to change an icon color to "currentColor" in order to inherit from text color.
    #[serde(default)]
    pub replace_attr_values: Option<LinkedHashMap<String, String>>,

    /// Specify a JSX runtime to use.
    /// * "classic": adds `import * as React from 'react'` on the top of file
    /// * "automatic": do not add anything
    /// * "classic-preact": adds `import { h } from 'preact'` on the top of file
    #[serde(default)]
    pub jsx_runtime: Option<JSXRuntime>,

    /// Specify a custom JSX runtime source to use. Allows to customize the import added at the top of generated file.
    #[serde(default)]
    pub jsx_runtime_import: Option<JSXRuntimeImport>,

    /// The named export defaults to `ReactComponent`, can be customized with the `named_export` option.
    #[serde(default = "default_named_export")]
    pub named_export: String,

    /// If you prefer named export in any case, you may set the `export_type` option to `named`.
    #[serde(default)]
    pub export_type: Option<ExportType>,
}

fn default_named_export() -> String {
    "ReactComponent".to_string()
}
