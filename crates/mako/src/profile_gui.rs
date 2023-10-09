use std::sync::Arc;

use eframe::egui;

use crate::compiler::Compiler;

pub struct ProfileApp {
    frame_counter: u64,
    compiler: Arc<Compiler>,
}

impl ProfileApp {
    #[allow(dead_code)]
    pub fn new(compiler: Arc<Compiler>) -> Self {
        Self {
            frame_counter: 0,
            compiler,
        }
    }
}

impl eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        puffin_egui::profiler_window(ctx);

        if self.frame_counter == 0 {
            self.compiler.compile();
        }

        self.frame_counter = 1;
    }
}
