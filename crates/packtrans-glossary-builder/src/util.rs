use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub fn http_client() -> ureq::Agent {
    let user_agent = format!(
        "packtrans/glossary/{} (https://github.com/packtrans/glossary)",
        env!("CARGO_PKG_VERSION")
    );
    ureq::AgentBuilder::new()
        .user_agent(&user_agent)
        .timeout(Duration::from_secs(30))
        .build()
}

pub fn progress_bar(len: u64, message: &'static str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({msg})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message);
    pb
}
