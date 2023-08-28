use glob::Pattern;

use crate::module::ModuleInfo;
use crate::resolve::{ResolvedResource, ResolverResource};

impl ModuleInfo {
    /**
     * 获取当前的模块是否具备 sideEffects
     */
    #[allow(dead_code)]
    pub fn get_side_effects_flag(&self) -> bool {
        if let Some(ResolverResource::Resolved(ResolvedResource(source))) = &self.resolved_resource
        {
            match &source.description {
                Some(desc) => {
                    let data = desc.data();
                    let value = data.raw();
                    let side_effects = value.get("sideEffects".to_string());

                    match side_effects {
                        Some(side_effect) => self.match_flag(side_effect),
                        None => true,
                    }
                }
                None => true,
            }
        } else {
            true
        }
    }
    #[allow(dead_code)]
    fn match_flag(&self, flag: &serde_json::Value) -> bool {
        match flag {
            // NOTE: 口径需要对齐这里：https://github.com/webpack/webpack/blob/main/lib/optimize/SideEffectsFlagPlugin.js#L331
            serde_json::Value::Bool(flag) => *flag,
            serde_json::Value::String(flag) => match_glob_pattern(flag, &self.path),
            serde_json::Value::Array(flags) => flags.iter().any(|flag| self.match_flag(flag)),
            _ => true,
        }
    }
}

#[allow(dead_code)]
fn match_glob_pattern(pattern: &str, path: &str) -> bool {
    // TODO: cache
    if !pattern.contains('/') {
        return Pattern::new(format!("**/{}", pattern).as_str())
            .unwrap()
            .matches(path);
    }
    Pattern::new(pattern).unwrap().matches(path)
}

#[cfg(test)]
mod tests {
    use crate::test_helper::{get_module, setup_compiler};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_side_effects_flag() {
        let compiler = setup_compiler("test/build/side-effects-flag", false);
        compiler.compile(None);
        let foo = get_module(&compiler, "node_modules/foo/index.ts");
        let bar = get_module(&compiler, "node_modules/bar/index.ts");
        let zzz = get_module(&compiler, "node_modules/zzz/index.ts");
        let four = get_module(&compiler, "node_modules/four/index.ts");
        let four_s = get_module(&compiler, "node_modules/four/index.s.ts");
        assert!(!foo.info.unwrap().get_side_effects_flag());
        assert!(bar.info.unwrap().get_side_effects_flag());
        assert!(zzz.info.unwrap().get_side_effects_flag());
        assert!(!four.info.unwrap().get_side_effects_flag());
        assert!(four_s.info.unwrap().get_side_effects_flag());
    }
}
