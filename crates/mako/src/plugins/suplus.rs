use std::sync::{Arc, Mutex};

use crate::plugin::{NextBuildParam, Plugin};

pub struct SUPlus {
    scanning: Arc<Mutex<bool>>,
}

impl SUPlus {
    pub fn new() -> Self {
        SUPlus {
            scanning: Arc::new(Mutex::new(true)),
        }
    }
}

impl Plugin for SUPlus {
    fn name(&self) -> &str {
        "suplus"
    }
    fn next_build(&self, _next_build_param: &NextBuildParam) -> bool {
        let x = self.scanning.lock().unwrap();

        if x.eq(&false) {
            return true;
        }

        // stop next file build, if from src to node_modules
        if !_next_build_param.current_module.id.contains("node_modules")
            && _next_build_param.next_file.is_under_node_modules
        {
            println!(
                "from {} -> to {}",
                _next_build_param.current_module.id,
                _next_build_param.next_file.pathname.to_string_lossy()
            );
            return false;
        }
        true
    }
}
