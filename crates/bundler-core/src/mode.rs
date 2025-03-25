use turbo_tasks::TaskInput;
use turbopack_ecmascript_runtime::RuntimeType;

/// The mode in which Next.js is running.
#[turbo_tasks::value(shared)]
#[derive(Debug, Copy, Clone, TaskInput, Ord, PartialOrd, Hash)]
pub enum Mode {
    /// `next dev --turbopack`
    Development,
    /// `next build`
    Build,
}

impl Mode {
    /// Returns the NODE_ENV value for the current mode.
    pub fn node_env(&self) -> &'static str {
        match self {
            Mode::Development => "development",
            Mode::Build => "production",
        }
    }

    /// Returns the exports condition for the current mode.
    pub fn condition(&self) -> &'static str {
        match self {
            Mode::Development => "development",
            Mode::Build => "production",
        }
    }

    /// Returns true if the development React runtime should be used.
    pub fn is_react_development(&self) -> bool {
        match self {
            Mode::Development => true,
            Mode::Build => false,
        }
    }

    pub fn is_development(&self) -> bool {
        match self {
            Mode::Development => true,
            Mode::Build => false,
        }
    }

    pub fn is_production(&self) -> bool {
        match self {
            Mode::Development => false,
            Mode::Build => true,
        }
    }

    pub fn runtime_type(&self) -> RuntimeType {
        match self {
            Mode::Development => RuntimeType::Development,
            Mode::Build => RuntimeType::Production,
        }
    }
}
