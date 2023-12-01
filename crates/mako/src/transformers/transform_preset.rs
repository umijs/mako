use std::collections::HashMap;

use mako_core::swc_common::comments::NoopComments;
use mako_core::swc_common::pass::Optional;
use mako_core::swc_common::{chain, Mark};
use mako_core::swc_ecma_ast::{EsVersion, Module};
use mako_core::swc_ecma_preset_env::{self as swc_preset_env};
use mako_core::swc_ecma_transforms::feature::FeatureFlag;
use mako_core::swc_ecma_transforms::{compat, Assumptions};
use mako_core::swc_ecma_visit::{Fold, VisitMut, VisitMutWith};

use crate::config::Targets;
use crate::targets;

pub struct Preset {
    pub targets: Targets,
    pub unresolved_mark: Mark,
}

impl VisitMut for Preset {
    fn visit_mut_module(&mut self, module: &mut Module) {
        let mut preset = match &self.targets {
            Targets::Env(env) => self.env_preset(env),
            Targets::EsVersion(es_version) => self.es_version_preset(es_version),
        };
        module.body = preset.fold_module(module.clone()).body;
        module.visit_mut_children_with(self);
    }
}

impl Preset {
    fn env_preset(&self, env: &HashMap<String, f32>) -> Box<dyn Fold> {
        Box::new(swc_preset_env::preset_env(
            self.unresolved_mark,
            Some(NoopComments),
            swc_preset_env::Config {
                mode: Some(swc_preset_env::Mode::Entry),
                targets: Some(targets::swc_preset_env_targets_from_map(env.clone())),
                ..Default::default()
            },
            Assumptions::default(),
            &mut FeatureFlag::default(),
        ))
    }

    fn es_version_preset(&self, es_version: &EsVersion) -> Box<dyn Fold> {
        let target = es_version;
        let assumptions = Assumptions::default();
        Box::new(chain!(
            Optional::new(
                compat::class_fields_use_set::class_fields_use_set(assumptions.pure_getters),
                assumptions.set_public_class_fields,
            ),
            Optional::new(
                compat::es2022::es2022(
                    Some(NoopComments),
                    compat::es2022::Config {
                        class_properties: compat::es2022::class_properties::Config {
                            private_as_properties: assumptions.private_fields_as_properties,
                            constant_super: assumptions.constant_super,
                            set_public_fields: assumptions.set_public_class_fields,
                            no_document_all: assumptions.no_document_all,
                            static_blocks_mark: Mark::new(),
                        }
                    }
                ),
                should_enable(target, &EsVersion::Es2022)
            ),
            Optional::new(
                compat::es2021::es2021(),
                should_enable(target, &EsVersion::Es2021)
            ),
            Optional::new(
                compat::es2020::es2020(
                    compat::es2020::Config {
                        nullish_coalescing: compat::es2020::nullish_coalescing::Config {
                            no_document_all: assumptions.no_document_all
                        },
                        optional_chaining: compat::es2020::optional_chaining::Config {
                            no_document_all: assumptions.no_document_all,
                            pure_getter: assumptions.pure_getters
                        }
                    },
                    self.unresolved_mark
                ),
                should_enable(target, &EsVersion::Es2020)
            ),
            Optional::new(
                compat::es2019::es2019(),
                should_enable(target, &EsVersion::Es2019)
            ),
            Optional::new(
                compat::es2018(compat::es2018::Config {
                    object_rest_spread: compat::es2018::object_rest_spread::Config {
                        no_symbol: assumptions.object_rest_no_symbols,
                        set_property: assumptions.set_spread_properties,
                        pure_getters: assumptions.pure_getters
                    }
                }),
                should_enable(target, &EsVersion::Es2018)
            ),
            Optional::new(
                compat::es2017(
                    compat::es2017::Config {
                        async_to_generator: compat::es2017::async_to_generator::Config {
                            ignore_function_name: assumptions.ignore_function_name,
                            ignore_function_length: assumptions.ignore_function_length
                        },
                    },
                    Some(NoopComments),
                    self.unresolved_mark
                ),
                should_enable(target, &EsVersion::Es2017)
            ),
            Optional::new(compat::es2016(), should_enable(target, &EsVersion::Es2016)),
            Optional::new(
                compat::es2015(
                    self.unresolved_mark,
                    Some(NoopComments),
                    compat::es2015::Config {
                        classes: compat::es2015::classes::Config {
                            constant_super: assumptions.constant_super,
                            no_class_calls: assumptions.no_class_calls,
                            set_class_methods: assumptions.set_class_methods,
                            super_is_callable_constructor: assumptions
                                .super_is_callable_constructor
                        },
                        computed_props: compat::es2015::computed_props::Config { loose: true },
                        for_of: compat::es2015::for_of::Config {
                            assume_array: false,
                            loose: true
                        },
                        spread: compat::es2015::spread::Config { loose: true },
                        destructuring: compat::es2015::destructuring::Config { loose: true },
                        regenerator: Default::default(),
                        template_literal: compat::es2015::template_literal::Config {
                            ignore_to_primitive: assumptions.ignore_to_primitive_hint,
                            mutable_template: assumptions.mutable_template_object
                        },
                        parameters: compat::es2015::parameters::Config {
                            ignore_function_length: assumptions.ignore_function_length,
                        },
                        typescript: true
                    }
                ),
                should_enable(target, &EsVersion::Es2015)
            ),
            Optional::new(
                compat::es3(true),
                cfg!(feature = "es3") && target == &EsVersion::Es3
            )
        ))
    }
}

fn should_enable(target: &EsVersion, feature: &EsVersion) -> bool {
    target < feature
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use mako_core::swc_common::{Mark, GLOBALS};
    use mako_core::swc_ecma_ast::EsVersion;

    use crate::compiler::Context;
    use crate::config::Targets;

    #[test]
    fn test_preset_env_1() {
        let mut map = HashMap::new();
        map.insert("chrome".to_string(), 91_f32);
        let input = r#"
const obj = {
    foo: {
        bar () {
            return 42;
        }
    }
};
const baz = obj?.foo?.bar?.();
        "#;
        transform(input, input, Targets::Env(map));
    }

    #[test]
    fn test_preset_env_2() {
        let mut map = HashMap::new();
        map.insert("chrome".to_string(), 90_f32);
        transform(
            r#"
const obj = {
    foo: {
        bar () {
            return 42;
        }
    }
};
const baz = obj?.foo?.bar?.();
        "#,
            r#"
var _obj_foo_bar, _obj_foo;
const obj = {
    foo: {
        bar () {
            return 42;
        }
    }
};
const baz = obj === null || obj === void 0 ? void 0 : _obj_foo = obj.foo === null || _obj_foo === void 0 ? void 0 : _obj_foo_bar = _obj_foo.bar === null || _obj_foo_bar === void 0 ? void 0 : _obj_foo_bar.call(_obj_foo);
        "#,
            Targets::Env(map),
        );
    }

    #[test]
    fn test_preset_es_version_1() {
        let input = r#"
const abc = 1;
const str = `${abc}_abc_${abc}`;
        "#;
        transform(input, input, Targets::EsVersion(EsVersion::Es2015));
    }

    #[test]
    fn test_preset_es_version_2() {
        let input = r#"
const abc = 1;
const str = `${abc}_abc_${abc}`;
        "#;
        transform(
            input,
            r#"
var abc = 1;
var str = "".concat(abc, "_abc_").concat(abc);
        "#,
            Targets::EsVersion(EsVersion::Es5),
        );
    }

    fn transform(code: &str, output: &str, targets: Targets) {
        let context: Arc<Context> = Arc::new(Context::default());

        GLOBALS.set(&context.meta.script.globals, || {
            let mut visitor = super::Preset {
                targets,
                unresolved_mark: Mark::new(),
            };
            let transformed = crate::transformers::test_helper::transform_js_code(
                code.trim(),
                &mut visitor,
                &context,
            );
            assert_eq!(output.trim(), transformed.trim());
        })
    }
}
