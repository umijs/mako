use std::path::PathBuf;

use glob::Pattern;
use glob_match::glob_match;

use crate::module::{relative_to_root, ModuleInfo};
use crate::resolve::{ResolvedResource, ResolverResource};

impl ModuleInfo {
    pub fn described_side_effect(&self) -> Option<bool> {
        if let Some(ResolverResource::Resolved(ResolvedResource(source))) = &self.resolved_resource
        {
            if let Some(root) = &source.pkg_root
                && let Some(side_effects_json_str) = &source.pkg_json.side_effects
            {
                let root: PathBuf = root.into();

                let side_effects = serde_json::from_str(side_effects_json_str).ok();

                side_effects.map(|side_effect| {
                    Self::match_flag(
                        &side_effect,
                        relative_to_root(&self.file.path.to_string_lossy().to_string(), &root)
                            .as_str(),
                    )
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn match_flag(flag: &serde_json::Value, path: &str) -> bool {
        match flag {
            // NOTE: 口径需要对齐这里：https://github.com/webpack/webpack/blob/main/lib/optimize/SideEffectsFlagPlugin.js#L331
            serde_json::Value::Bool(flag) => *flag,
            serde_json::Value::String(flag) => match_glob_pattern(flag, path),
            serde_json::Value::Array(flags) => {
                flags.iter().any(|flag| Self::match_flag(flag, path))
            }
            _ => true,
        }
    }
}

fn match_glob_pattern(pattern: &str, path: &str) -> bool {
    let trimmed = path.trim_start_matches("./");

    // TODO: cache
    if !pattern.contains('/') {
        return Pattern::new(format!("**/{}", pattern).as_str())
            .unwrap_or_else(|_| panic!("the pattern is invalid **/{} for path {path}", pattern))
            .matches(trimmed);
    }

    glob_match(pattern.trim_start_matches("./"), trimmed)
}

#[cfg(test)]
mod tests {
    use super::match_glob_pattern;
    use crate::utils::test_helper::{get_module, setup_compiler};

    #[test]
    fn test_path_side_effects_no_dot_start_pattern() {
        assert!(match_glob_pattern("esm/index.js", "./esm/index.js",));
    }

    #[test]
    fn test_exact_path_side_effects_flag() {
        assert!(match_glob_pattern("./src/index.js", "./src/index.js",));
    }

    #[test]
    fn test_exact_path_side_effects_flag_negative() {
        assert!(!match_glob_pattern("./src/index.js", "./dist/index.js",));
    }

    #[test]
    fn test_wild_effects_flag() {
        assert!(match_glob_pattern(
            "./src/lib/**/*.s.js",
            "./src/lib/apple/pie/index.s.js",
        ));
    }

    #[test]
    fn test_double_wild_starts_effects_flag() {
        assert!(match_glob_pattern(
            "**/index.js",
            "./deep/lib/file/index.js",
        ));
    }

    #[test]
    fn test_side_effects_flag() {
        let compiler = setup_compiler("test/build/side-effects-flag", false);
        compiler.compile().unwrap();
        let foo = get_module(&compiler, "node_modules/foo/index.ts");
        let bar = get_module(&compiler, "node_modules/bar/index.ts");
        let zzz = get_module(&compiler, "node_modules/zzz/index.ts");
        let four = get_module(&compiler, "node_modules/four/index.ts");
        let four_s = get_module(&compiler, "node_modules/four/index.s.ts");
        assert_eq!(foo.info.unwrap().described_side_effect(), Some(false));
        assert_eq!(bar.info.unwrap().described_side_effect(), None);
        assert_eq!(zzz.info.unwrap().described_side_effect(), Some(true));
        assert_eq!(four.info.unwrap().described_side_effect(), Some(false));
        assert_eq!(four_s.info.unwrap().described_side_effect(), Some(true));
    }
}
