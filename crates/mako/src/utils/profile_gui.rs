#[cfg(feature = "profile")]
use std::sync::Arc;

#[cfg(feature = "profile")]
use mako_core::eframe::egui;
#[cfg(feature = "profile")]
use mako_core::tokio::sync::Notify;

use crate::compiler::Compiler;

#[cfg(feature = "profile")]
pub struct ProfileApp {
    notified: bool,
    compiler: Arc<Compiler>,
    notify: Arc<Notify>,
}

#[cfg(feature = "profile")]
impl ProfileApp {
    #[allow(dead_code)]
    pub fn new(notify: Arc<Notify>, compiler: Arc<Compiler>) -> Self {
        Self {
            notified: false,
            notify,
            compiler,
        }
    }
}

#[cfg(feature = "profile")]
impl mako_core::eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut mako_core::eframe::Frame) {
        mako_core::puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        if !self.notified {
            if self.compiler.context.args.watch {
                self.notify.notify_one();
            } else {
                self.compiler.compile().unwrap();
            }
            self.notified = true;
        }
        mako_core::puffin_egui::profiler_window(ctx);
    }
}
