pub const APP_NAME: &str = "🌖 utoo";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_ABOUT: &str = "/juːtuː/ Unified Toolchain: Open & Optimized";

pub mod cmd {
    pub const CLEAN_NAME: &str = "clean";
    pub const CLEAN_ABOUT: &str = "Clean package cache in global storage";
    pub const DEPS_NAME: &str = "deps";
    pub const DEPS_ABOUT: &str = "List and analyze project dependencies";
    pub const INSTALL_NAME: &str = "install";
    pub const INSTALL_ABOUT: &str = "Install project dependencies";
    pub const REBUILD_NAME: &str = "rebuild";
    pub const REBUILD_ABOUT: &str = "Rebuild native modules";
}
