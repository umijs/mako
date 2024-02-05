use mako_core::collections::HashMap;
use mako_core::swc_ecma_preset_env::Targets as SwcPresetEnvTargets;

pub fn swc_preset_env_targets_from_map(map: HashMap<String, f32>) -> SwcPresetEnvTargets {
    let serialized_str = serde_json::to_string(&map).unwrap();
    let targets: SwcPresetEnvTargets = serde_json::from_str(&serialized_str).unwrap();
    targets
}
