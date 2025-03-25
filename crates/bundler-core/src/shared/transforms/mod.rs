pub use modularize_imports::ModularizeImportPackageConfig;
use turbo_tasks::ResolvedVc;
use turbopack::module_options::{ModuleRule, ModuleRuleEffect, RuleCondition};
use turbopack_core::reference_type::{ReferenceType, UrlReferenceSubType};
use turbopack_ecmascript::{CustomTransformer, EcmascriptInputTransform};

pub mod emotion;
pub mod modularize_imports;
pub mod remove_console;
pub mod styled_components;
pub mod styled_jsx;
pub mod swc_ecma_transform_plugins;

fn match_js_extension(enable_mdx_rs: bool) -> Vec<RuleCondition> {
    let mut conditions = vec![
        RuleCondition::ResourcePathEndsWith(".js".to_string()),
        RuleCondition::ResourcePathEndsWith(".jsx".to_string()),
        RuleCondition::All(vec![
            RuleCondition::ResourcePathEndsWith(".ts".to_string()),
            RuleCondition::Not(Box::new(RuleCondition::ResourcePathEndsWith(
                ".d.ts".to_string(),
            ))),
        ]),
        RuleCondition::ResourcePathEndsWith(".tsx".to_string()),
        RuleCondition::ResourcePathEndsWith(".mjs".to_string()),
        RuleCondition::ResourcePathEndsWith(".cjs".to_string()),
    ];

    if enable_mdx_rs {
        conditions.append(
            vec![
                RuleCondition::ResourcePathEndsWith(".md".to_string()),
                RuleCondition::ResourcePathEndsWith(".mdx".to_string()),
            ]
            .as_mut(),
        );
    }
    conditions
}

/// Returns a module rule condition matches to any ecmascript (with mdx if
/// enabled) except url reference type. This is a typical custom rule matching
/// condition for custom ecma specific transforms.
pub(crate) fn module_rule_match_js_no_url(enable_mdx_rs: bool) -> RuleCondition {
    let conditions = match_js_extension(enable_mdx_rs);

    RuleCondition::all(vec![
        RuleCondition::not(RuleCondition::ReferenceType(ReferenceType::Url(
            UrlReferenceSubType::Undefined,
        ))),
        RuleCondition::any(conditions),
    ])
}

/// Create a new module rule for the given ecmatransform, runs against
/// any ecmascript (with mdx if enabled) except url reference type
pub(crate) fn get_ecma_transform_rule(
    transformer: Box<dyn CustomTransformer + Send + Sync>,
    enable_mdx_rs: bool,
    prepend: bool,
) -> ModuleRule {
    let transformer = EcmascriptInputTransform::Plugin(ResolvedVc::cell(transformer as _));
    let (prepend, append) = if prepend {
        (
            ResolvedVc::cell(vec![transformer]),
            ResolvedVc::cell(vec![]),
        )
    } else {
        (
            ResolvedVc::cell(vec![]),
            ResolvedVc::cell(vec![transformer]),
        )
    };

    ModuleRule::new(
        module_rule_match_js_no_url(enable_mdx_rs),
        vec![ModuleRuleEffect::ExtendEcmascriptTransforms { prepend, append }],
    )
}
