use std::collections::HashMap;

use mako_core::swc_ecma_preset_env::Targets as SwcPresetEnvTargets;

pub fn swc_preset_env_targets_from_map(map: HashMap<String, f32>) -> SwcPresetEnvTargets {
    let serialized_str = serde_json::to_string(&map).unwrap();
    let targets: SwcPresetEnvTargets = serde_json::from_str(&serialized_str).unwrap();
    targets
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::swc_preset_env_targets_from_map;
    use crate::assert_debug_snapshot;

    #[test]
    fn test_swc_preset_env_targets() {
        let map = HashMap::from([("chrome".to_string(), 80.0)]);
        let targets = swc_preset_env_targets_from_map(map);
        assert_debug_snapshot!(&targets);
    }
}
