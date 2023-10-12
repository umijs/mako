use std::sync::Arc;

#[cfg(feature = "profile")]
use mako_core::eframe::egui;

use crate::compiler::Compiler;

pub struct ProfileApp {
    #[allow(dead_code)]
    frame_counter: u64,
    #[allow(dead_code)]
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

#[cfg(feature = "profile")]
impl mako_core::eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut mako_core::eframe::Frame) {
        mako_core::puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        mako_core::puffin_egui::profiler_window(ctx);

        if self.frame_counter == 0 {
            self.compiler.compile().unwrap();
        }

        self.frame_counter += 1;
    }
}
