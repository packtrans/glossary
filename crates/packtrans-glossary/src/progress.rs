use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

static SUPPRESS_SPINNERS: AtomicBool = AtomicBool::new(false);

/// When `true`, progress spinners are no-ops (used by MCP stdio transport).
pub fn set_suppress_spinners(suppress: bool) {
    SUPPRESS_SPINNERS.store(suppress, Ordering::Relaxed);
}

pub fn spinner(message: impl Into<String>) -> ProgressBar {
    if SUPPRESS_SPINNERS.load(Ordering::Relaxed) {
        let pb = ProgressBar::hidden();
        pb.set_message(message.into());
        return pb;
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.into());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}
