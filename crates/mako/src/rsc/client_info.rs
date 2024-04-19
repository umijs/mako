use mako_core::serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct RscClientInfo {
    pub path: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct RscCssModules {
    pub path: String,
}
