use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub fn spinner(message: impl Into<String>) -> ProgressBar {
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
