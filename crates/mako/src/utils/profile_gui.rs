#[cfg(feature = "profile")]
use std::sync::Arc;

#[cfg(feature = "profile")]
use mako_core::eframe::egui;
#[cfg(feature = "profile")]
use mako_core::tokio::sync::Notify;

#[cfg(feature = "profile")]
pub struct ProfileApp {
    notified: bool,
    notify: Arc<Notify>,
}

#[cfg(feature = "profile")]
impl ProfileApp {
    #[allow(dead_code)]
    pub fn new(notify: Arc<Notify>) -> Self {
        Self {
            notified: false,
            notify,
        }
    }
}

#[cfg(feature = "profile")]
impl mako_core::eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut mako_core::eframe::Frame) {
        mako_core::puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        if !self.notified {
            self.notified = true;
            self.notify.notify_one();
        }
        mako_core::puffin_egui::profiler_window(ctx);
    }
}
