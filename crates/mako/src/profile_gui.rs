#[cfg(feature = "profile")]
use mako_core::eframe::egui;

#[cfg(feature = "profile")]
pub struct ProfileApp {}

#[cfg(feature = "profile")]
impl mako_core::eframe::App for ProfileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut mako_core::eframe::Frame) {
        mako_core::puffin::GlobalProfiler::lock().new_frame(); // call once per frame!

        mako_core::puffin_egui::profiler_window(ctx);
    }
}
