use std::sync::Arc;

use eframe::egui;

use crate::compiler::Compiler;
use crate::utils::tokio_runtime;

pub struct ProfileApp {
    inited: bool,
    compiler: Arc<Compiler>,
}

impl ProfileApp {
    pub fn new(compiler: Arc<Compiler>) -> Self {
        Self {
            inited: false,
            compiler,
        }
    }
}

impl eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        if !self.inited {
            self.compiler.compile().unwrap();

            if self.compiler.context.args.watch {
                let for_spawn = self.compiler.clone();
                tokio_runtime::spawn(async move {
                    let root = for_spawn.context.root.clone();
                    let d = crate::dev::DevServer::new(root, for_spawn);
                    d.serve(move |_params| {}).await;
                });
            }
            self.inited = true;
        }
        puffin_egui::profiler_window(ctx);
    }
}
