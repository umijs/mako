use turbo_tasks::OperationValue;
use turbopack_ecmascript_runtime::RuntimeType;

/// The mode in which Next.js is running.
#[turbo_tasks::value(shared)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Hash, OperationValue)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Development,
    Production,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Production
    }
}

impl Mode {
    /// Returns the NODE_ENV value for the current mode.
    pub fn node_env(&self) -> &'static str {
        match self {
            Mode::Development => "development",
            Mode::Production => "production",
        }
    }

    /// Returns the exports condition for the current mode.
    pub fn condition(&self) -> &'static str {
        match self {
            Mode::Development => "development",
            Mode::Production => "production",
        }
    }

    /// Returns true if the development React runtime should be used.
    pub fn is_react_development(&self) -> bool {
        match self {
            Mode::Development => true,
            Mode::Production => false,
        }
    }

    pub fn is_development(&self) -> bool {
        match self {
            Mode::Development => true,
            Mode::Production => false,
        }
    }

    pub fn is_production(&self) -> bool {
        match self {
            Mode::Development => false,
            Mode::Production => true,
        }
    }

    pub fn runtime_type(&self) -> RuntimeType {
        match self {
            Mode::Development => RuntimeType::Development,
            Mode::Production => RuntimeType::Production,
        }
    }
}
