use std::collections::HashMap;

use lightningcss::targets::{Browsers, Targets as LightningcssTargets};
use swc_ecma_preset_env::Targets as SwcPresetEnvTargets;

pub fn swc_preset_env_targets_from_map(map: HashMap<String, usize>) -> SwcPresetEnvTargets {
    let serialized_str = serde_json::to_string(&map).unwrap();
    let targets: SwcPresetEnvTargets = serde_json::from_str(&serialized_str).unwrap();
    targets
}

pub fn lightningcss_targets_from_map(map: HashMap<String, usize>) -> LightningcssTargets {
    let mut browsers = Browsers::default();
    for (key, value) in map {
        let major: u32 = value.try_into().unwrap();
        // only consider the major version, the version format is (major & 0xff) << 16 | (minor & 0xff) << 8 | (patch & 0xff)
        let version = (major & 0xff) << 16;
        match key.as_str() {
            "android" => browsers.android.replace(version),
            "chrome" => browsers.chrome.replace(version),
            "edge" => browsers.edge.replace(version),
            "firefox" => browsers.firefox.replace(version),
            "ie" => browsers.ie.replace(version),
            "ios_saf" => browsers.ios_saf.replace(version),
            "opera" => browsers.opera.replace(version),
            "safari" => browsers.safari.replace(version),
            "samsung" => browsers.samsung.replace(version),
            _ => None,
        };
    }
    LightningcssTargets::from(browsers)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{lightningcss_targets_from_map, swc_preset_env_targets_from_map};
    use crate::assert_debug_snapshot;

    #[test]
    fn test_swc_preset_env_targets() {
        let map = HashMap::from([("chrome".to_string(), 80)]);
        let targets = swc_preset_env_targets_from_map(map);
        assert_debug_snapshot!(&targets);
    }

    #[test]
    fn test_lightningcss_targets() {
        let map = HashMap::from([("chrome".to_string(), 80)]);
        let targets = lightningcss_targets_from_map(map);
        assert_debug_snapshot!(&targets);
    }
}
