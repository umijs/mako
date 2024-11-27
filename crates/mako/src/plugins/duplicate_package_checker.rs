use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use semver::Version;

use crate::compiler::Context;
use crate::module::Module;
use crate::module_graph::ModuleGraph;
use crate::plugin::Plugin;
use crate::resolve::ResolverResource;

#[derive(Debug, Clone)]
struct PackageInfo {
    name: String,
    version: Version,
    path: PathBuf,
}

#[derive(Default)]
pub struct DuplicatePackageCheckerPlugin {
    verbose: bool,
    show_help: bool,
    emit_error: bool,
}

/// Cleans the path by replacing /node_modules/ or \node_modules\ with /~/
fn clean_path(module_path: &Path) -> PathBuf {
    let path_str = module_path.to_string_lossy();
    let cleaned_path = path_str
        .replace("/node_modules/", "/~/")
        .replace("\\node_modules\\", "/~/");
    PathBuf::from(cleaned_path)
}

/// Makes the cleaned path relative to the given context
fn clean_path_relative_to_context(module_path: &Path, context: &Path) -> PathBuf {
    let cleaned_path = clean_path(module_path);
    let context_str = context.to_str().unwrap();
    let cleaned_path_str = cleaned_path.to_str().unwrap();

    if cleaned_path_str.starts_with(context_str) {
        let relative_path = cleaned_path_str.trim_start_matches(context_str);
        PathBuf::from(format!(".{}", relative_path))
    } else {
        cleaned_path
    }
}

fn extract_package_info(module: &Module) -> Option<PackageInfo> {
    module
        .info
        .as_ref()
        .and_then(|info| info.resolved_resource.as_ref())
        .and_then(|resolver_resource| {
            if let ResolverResource::Resolved(resource) = resolver_resource {
                let package_json = resource.0.package_json()?;
                let name = package_json.name.clone()?;
                let raw_json = package_json.raw_json();
                let version = raw_json.as_object()?.get("version")?;
                let version = semver::Version::parse(version.as_str().unwrap()).ok()?;

                Some(PackageInfo {
                    name,
                    version,
                    path: package_json.path.clone(),
                })
            } else {
                None
            }
        })
}

impl DuplicatePackageCheckerPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn show_help(mut self, show_help: bool) -> Self {
        self.show_help = show_help;
        self
    }

    pub fn emit_error(mut self, emit_error: bool) -> Self {
        self.emit_error = emit_error;
        self
    }

    fn find_duplicates(packages: Vec<PackageInfo>) -> HashMap<String, Vec<PackageInfo>> {
        let mut package_map: HashMap<String, Vec<PackageInfo>> = HashMap::new();

        for package in packages {
            package_map
                .entry(package.name.clone())
                .or_default()
                .push(package);
        }

        package_map
            .into_iter()
            .filter(|(_, versions)| versions.len() > 1)
            .collect()
    }

    fn check_duplicates(
        &self,
        module_graph: &RwLock<ModuleGraph>,
    ) -> HashMap<String, Vec<PackageInfo>> {
        let mut packages = Vec::new();

        module_graph
            .read()
            .unwrap()
            .modules()
            .into_iter()
            .filter_map(extract_package_info)
            .for_each(|package_info| {
                packages.push(package_info);
            });

        Self::find_duplicates(packages)
    }
}

impl Plugin for DuplicatePackageCheckerPlugin {
    fn name(&self) -> &str {
        "DuplicatePackageCheckerPlugin"
    }

    fn after_build(
        &self,
        context: &Arc<Context>,
        _compiler: &crate::compiler::Compiler,
    ) -> anyhow::Result<()> {
        let duplicates = self.check_duplicates(&context.module_graph);

        if !duplicates.is_empty() && self.verbose {
            let mut message = String::new();

            for (name, instances) in duplicates {
                message.push_str(&format!("\nMultiple versions of {} found:\n", name));
                for instance in instances {
                    let mut line = format!("  {} {}", instance.version, instance.name);
                    let path = instance.path.clone();
                    line.push_str(&format!(
                        " from {}",
                        clean_path_relative_to_context(&path, &context.root).display()
                    ));
                    message.push_str(&line);
                    message.push('\n');
                }
            }

            if self.show_help {
                message.push_str("\nCheck how you can resolve duplicate packages: \nhttps://github.com/darrenscerri/duplicate-package-checker-webpack-plugin#resolving-duplicate-packages-in-your-bundle\n");
            }

            if !self.emit_error {
                println!("{}", message);
            } else {
                eprintln!("{}", message);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::{Config, DuplicatePackageCheckerConfig};
    use crate::plugin::Plugin;
    use crate::plugins::duplicate_package_checker::DuplicatePackageCheckerPlugin;
    use crate::utils::test_helper::setup_compiler;

    #[test]
    fn test_duplicate_package_checker() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test/build/duplicate-package");
        let mut config = Config::new(&root, None, None).unwrap();
        config.check_duplicate_package = Some(DuplicatePackageCheckerConfig {
            verbose: true,
            emit_error: false,
            show_help: true,
        });

        let compiler = setup_compiler("test/build/duplicate-package", false);
        let plugin = DuplicatePackageCheckerPlugin::new()
            .verbose(true)
            .show_help(true)
            .emit_error(false);

        // 运行编译
        compiler.compile().unwrap();

        // 执行插件的 after_build 方法
        let result = plugin.after_build(&compiler.context, &compiler);

        assert!(result.is_ok());
    }
}
