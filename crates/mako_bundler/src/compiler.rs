use crate::{
    build::build::BuildParam, config::Config, context::Context, generate::generate::GenerateParam,
};
use std::sync::{Arc, Mutex};

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(config: Config) -> Self {
        let context = Context::new(config);
        Self {
            context: Arc::new(context),
        }
    }

    pub fn run(&mut self) {
        let context = Arc::get_mut(&mut self.context).unwrap();
        let config = Arc::new(Mutex::new(&mut context.config));

        // allow plugin to modify config
        context
            .plugin_driver
            .run_hook_serial(|p, _last_ret| {
                let mut config_lock = config.lock().unwrap();

                p.config(&mut config_lock)?;
                Ok(Some(()))
            })
            .unwrap();

        self.build(&BuildParam { files: None });
        self.generate(&GenerateParam { write: true });
    }

    pub fn _watch(&self) {}
}
