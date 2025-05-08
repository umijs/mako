use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub static PROGRESS_BAR: Lazy<ProgressBar> = Lazy::new(|| {
    let pb = ProgressBar::new(0).with_style(
        ProgressStyle::with_template("{spinner:.blue} +{pos:.green} ~{len:.magenta} {wide_msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    pb
});

pub fn finish_progress_bar(msg: &str) {
    PROGRESS_BAR.set_style(
        ProgressStyle::with_template("✓ +{pos:.green} ~{len:.magenta} {wide_msg}").unwrap(),
    );
    PROGRESS_BAR.finish_with_message(msg.to_string());
    PROGRESS_BAR.set_draw_target(indicatif::ProgressDrawTarget::hidden());
}

pub fn start_progress_bar() {
    PROGRESS_BAR.reset();
    PROGRESS_BAR.set_style(
        ProgressStyle::with_template("{spinner:.blue} +{pos:.green} ~{len:.magenta} {wide_msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    PROGRESS_BAR.set_draw_target(indicatif::ProgressDrawTarget::stderr());
    PROGRESS_BAR.enable_steady_tick(Duration::from_millis(100));
}

// add a global variable to store the verbose mode
static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(verbose: bool) {
    VERBOSE.store(verbose, Ordering::Relaxed);
    log_verbose("verbose mode enabled");
}

// temp log in memory
static VERBOSE_LOGS: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

use crate::util::timer::Timer;

pub fn log_verbose(msg: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        println!("🔍 {}\x1b[0m", msg);
    }
    if let Ok(mut logs) = VERBOSE_LOGS.lock() {
        logs.push(format!("[{}][VERBOSE] {}", Timer::format_datetime(), msg));
    }
}

pub fn get_verbose_logs() -> Vec<String> {
    VERBOSE_LOGS
        .lock()
        .map(|logs| logs.clone())
        .unwrap_or_default()
}

pub fn log_warning(text: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        PROGRESS_BAR.suspend(|| println!("[WARNING] {}", text));
    } else {
        PROGRESS_BAR.suspend(|| println!("{} {}\x1b[0m", " WARNING ".on_yellow(), text));
    }
    if let Ok(mut logs) = VERBOSE_LOGS.lock() {
        logs.push(format!("[{}][WARNING] {}", Timer::format_datetime(), text));
    }
}

pub fn log_error(text: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        PROGRESS_BAR.suspend(|| println!("[ERROR] {}", text));
    } else {
        PROGRESS_BAR.suspend(|| println!("{} {}\x1b[0m", " ERROR ".on_red(), text));
    }
    if let Ok(mut logs) = VERBOSE_LOGS.lock() {
        logs.push(format!("[{}][ERROR] {}", Timer::format_datetime(), text));
    }
}

pub fn log_info(text: &str) {
    if VERBOSE.load(Ordering::Relaxed) {
        PROGRESS_BAR.suspend(|| println!("[INFO] {}", text));
    } else {
        PROGRESS_BAR.suspend(|| println!("{} {}\x1b[0m", " INFO ".on_cyan(), text));
    }
    if let Ok(mut logs) = VERBOSE_LOGS.lock() {
        logs.push(format!("[{}][INFO] {}", Timer::format_datetime(), text));
    }
}

pub fn log_progress(text: &str) {
    PROGRESS_BAR.set_message(text.to_string());
    // log_verbose(text);
}

pub fn write_verbose_logs_to_file() -> std::io::Result<String> {
    self::finish_progress_bar("done");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let log_file = env::temp_dir()
        .join(format!("utoo-{}.log", timestamp))
        .to_string_lossy()
        .to_string();

    let logs = get_verbose_logs();
    if !logs.is_empty() {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_file)?;

        file.write_all(logs.join("\n").as_bytes())?;

        log_error(&format!("Verbose logs have been saved to {}", log_file));
    }
    Ok(log_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_verbose_true() {
        set_verbose(true);
        assert!(VERBOSE.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_verbose_false() {
        set_verbose(false);
        assert!(!VERBOSE.load(Ordering::Relaxed));
    }

    #[test]
    fn test_set_verbose_multiple_calls() {
        set_verbose(true);
        assert!(VERBOSE.load(Ordering::Relaxed));

        set_verbose(false);
        assert!(!VERBOSE.load(Ordering::Relaxed));

        set_verbose(true);
        assert!(VERBOSE.load(Ordering::Relaxed));
    }
}
