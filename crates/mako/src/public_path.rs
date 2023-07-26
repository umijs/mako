use crate::config::Config;

// build_stage 的 public_path 获取
pub fn build_stage_public_path(config: &Config) -> String {
    if config.public_path != "runtime" {
        config.public_path.clone()
    } else {
        "".to_owned()
    }
}

// runtime 的 public_path 获取
pub fn runtime_public_path(config: &Config) -> String {
    if config.public_path == "runtime" {
        "globalThis.publicPath".to_string()
    } else {
        config.public_path.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::public_path::{build_stage_public_path, runtime_public_path};

    #[test]
    fn test_default_public_path() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(&current_dir.join("test/config/normal"), None, None).unwrap();
        let build_stage_public_path = build_stage_public_path(&config);
        assert_eq!(build_stage_public_path, "/");

        let runtime_public_path = runtime_public_path(&config);
        assert_eq!(runtime_public_path, "/");
    }

    #[test]
    fn test_public_path() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(&current_dir.join("test/config/public_path"), None, None).unwrap();
        let build_stage_public_path = build_stage_public_path(&config);
        assert_eq!(build_stage_public_path, "https://umijs.com/");

        let runtime_public_path = runtime_public_path(&config);
        assert_eq!(runtime_public_path, "https://umijs.com/");
    }

    #[test]
    fn test_public_path_runtime() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(
            &current_dir.join("test/config/public_path_runtime"),
            None,
            None,
        )
        .unwrap();
        let build_stage_public_path = build_stage_public_path(&config);
        assert_eq!(build_stage_public_path, "");

        let runtime_public_path = runtime_public_path(&config);
        assert_eq!(runtime_public_path, "globalThis.publicPath");
    }
}
